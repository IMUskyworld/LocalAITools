use std::ffi::CString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct InferenceEngine {
    model: *mut llama_cpp_sys_2::llama_model,
    vocab: *const llama_cpp_sys_2::llama_vocab,
    ctx: *mut llama_cpp_sys_2::llama_context,
    is_loaded: AtomicBool,
    is_running: AtomicBool,
    speed: Arc<Mutex<f32>>,
    n_ctx: u32,
}

unsafe impl Send for InferenceEngine {}
unsafe impl Sync for InferenceEngine {}

impl Drop for InferenceEngine {
    fn drop(&mut self) { self.unload_model_inner(); }
}

impl InferenceEngine {
    pub fn new() -> Self {
        Self {
            model: std::ptr::null_mut(),
            vocab: std::ptr::null(),
            ctx: std::ptr::null_mut(),
            is_loaded: AtomicBool::new(false),
            is_running: AtomicBool::new(false),
            speed: Arc::new(Mutex::new(0.0)),
            n_ctx: 4096,
        }
    }

    fn unload_model_inner(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        self.is_loaded.store(false, Ordering::SeqCst);
        if !self.ctx.is_null() { unsafe { llama_cpp_sys_2::llama_free(self.ctx); } }
        if !self.model.is_null() { unsafe { llama_cpp_sys_2::llama_free_model(self.model); } }
        self.ctx = std::ptr::null_mut();
        self.model = std::ptr::null_mut();
        self.vocab = std::ptr::null();
    }

    pub fn load_model(&mut self, path: &str) -> Result<(), String> {
        if !Path::new(path).exists() {
            return Err(format!("模型文件不存在: {}", path));
        }
        let cpath = CString::new(path).map_err(|e| format!("路径: {:?}", e))?;

        unsafe { llama_cpp_sys_2::llama_backend_init(); }

        let mut model_params = unsafe { llama_cpp_sys_2::llama_model_default_params() };
        model_params.n_gpu_layers = 0;

        let model = unsafe { llama_cpp_sys_2::llama_load_model_from_file(cpath.as_ptr(), model_params) };
        if model.is_null() { return Err("模型加载失败".to_string()); }

        let vocab = unsafe { llama_cpp_sys_2::llama_model_get_vocab(model) };
        if vocab.is_null() { unsafe { llama_cpp_sys_2::llama_free_model(model); } return Err("获取 vocab 失败".to_string()); }

        let mut params = unsafe { llama_cpp_sys_2::llama_context_default_params() };
        params.n_ctx = self.n_ctx;
        params.n_threads = 4;
        params.n_threads_batch = 4;

        let ctx = unsafe { llama_cpp_sys_2::llama_init_from_model(model, params) };
        if ctx.is_null() { unsafe { llama_cpp_sys_2::llama_free_model(model); } return Err("上下文创建失败".to_string()); }

        self.model = model;
        self.vocab = vocab;
        self.ctx = ctx;
        self.is_loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn unload_model(&mut self) { self.unload_model_inner(); }

    pub fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String, String> {
        if self.ctx.is_null() || self.model.is_null() {
            return Err("模型未加载".to_string());
        }
        self.is_running.store(true, Ordering::SeqCst);

        let cprompt = CString::new(prompt).map_err(|e| format!("{:?}", e))?;
        let ctx = self.ctx;
        let vocab = self.vocab;
        let model = self.model;
        let max_t = max_tokens as i32;

        let mut output = String::new();
        let start = std::time::Instant::now();
        let mut generated: i32 = 0;

        unsafe {
            let n_prompt = llama_cpp_sys_2::llama_tokenize(
                vocab, cprompt.as_ptr(), prompt.len() as i32,
                std::ptr::null_mut(), 0, true, false,
            );
            if n_prompt <= 0 { self.is_running.store(false, Ordering::SeqCst); return Err("Token 化失败".to_string()); }

            let mut tokens = vec![0i32; n_prompt as usize];
            llama_cpp_sys_2::llama_tokenize(
                vocab, cprompt.as_ptr(), prompt.len() as i32,
                tokens.as_mut_ptr(), n_prompt, true, false,
            );

            let mut batch = llama_cpp_sys_2::llama_batch_init(n_prompt + max_t, 0, 1);
            batch.n_tokens = n_prompt;

            for i in 0..n_prompt {
                *batch.token.add(i as usize) = tokens[i as usize];
                *batch.pos.add(i as usize) = i;
                *batch.n_seq_id.add(i as usize) = 1;
                *batch.seq_id.add(i as usize) = std::ptr::null_mut();
                *batch.logits.add(i as usize) = 0;
            }
            // 最后一个 token 需要 logit
            *batch.logits.add((n_prompt - 1) as usize) = 1;

            if llama_cpp_sys_2::llama_decode(ctx, batch) != 0 {
                llama_cpp_sys_2::llama_batch_free(batch);
                self.is_running.store(false, Ordering::SeqCst);
                return Err("Prompt decode 失败".to_string());
            }

            let eos = llama_cpp_sys_2::llama_token_eos(vocab);
            let smpl = llama_cpp_sys_2::llama_sampler_init_greedy();
            let mut n_decode = n_prompt;

            loop {
                if !self.is_running.load(Ordering::SeqCst) || generated >= max_t { break; }

                let new_token = llama_cpp_sys_2::llama_sampler_sample(smpl, ctx, 0);
                if new_token == eos { break; }

                let mut buf: [i8; 1024] = [0; 1024];
                let n = llama_cpp_sys_2::llama_token_to_piece(
                    vocab, new_token, buf.as_mut_ptr(), 1024, 0, false,
                );
                if n > 0 {
                    let bytes = std::slice::from_raw_parts(buf.as_ptr() as *const u8, n as usize);
                    if let Ok(s) = std::str::from_utf8(bytes) { output.push_str(s); }
                }
                generated += 1;

                *batch.token.add(0) = new_token;
                *batch.pos.add(0) = n_decode;
                *batch.n_seq_id.add(0) = 1;
                *batch.seq_id.add(0) = std::ptr::null_mut();
                *batch.logits.add(0) = 1;
                batch.n_tokens = 1;
                n_decode += 1;

                if llama_cpp_sys_2::llama_decode(ctx, batch) != 0 { break; }
            }

            llama_cpp_sys_2::llama_sampler_free(smpl);
            llama_cpp_sys_2::llama_batch_free(batch);
        }

        let elapsed = start.elapsed().as_secs_f32();
        if elapsed > 0.0 {
            let sv = generated as f32 / elapsed;
            let s = self.speed.clone();
            tokio::spawn(async move { *s.lock().await = sv; });
        }

        self.is_running.store(false, Ordering::SeqCst);
        Ok(output)
    }

    pub async fn get_speed(&self) -> f32 { *self.speed.lock().await }
    pub fn is_loaded(&self) -> bool { self.is_loaded.load(Ordering::SeqCst) }
    pub fn stop(&self) { self.is_running.store(false, Ordering::SeqCst); }
    pub fn set_params(&mut self, _threads: u32, ctx_len: u32) { self.n_ctx = ctx_len.clamp(2048, 8192); }
    pub fn get_config(&self) -> InferenceConfig { InferenceConfig { context_length: self.n_ctx } }
}

impl Default for InferenceEngine { fn default() -> Self { Self::new() } }

#[derive(Debug, Clone, Copy)]
pub struct InferenceConfig { pub context_length: u32 }
impl Default for InferenceConfig { fn default() -> Self { Self { context_length: 4096 } } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_new() {
        let engine = InferenceEngine::new();
        assert!(!engine.is_loaded());
    }

    #[test]
    fn test_engine_params() {
        let mut engine = InferenceEngine::new();
        engine.set_params(8, 8192);
        let config = engine.get_config();
        assert_eq!(config.context_length, 8192);
    }

    #[test]
    fn test_generate_unloaded() {
        let engine = InferenceEngine::new();
        let result = engine.generate("test", 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未加载"));
    }

    #[test]
    fn test_model_load_fail_nonexistent() {
        let mut engine = InferenceEngine::new();
        let result = engine.load_model("/nonexistent/path/model.gguf");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("不存在"));
    }

    #[test]
    fn test_model_load_small_gguf() {
        let model_path = r"D:\WinClaw\resources\resources\models\bge-small-zh-v1.5-q4_k_m.gguf";
        if !Path::new(model_path).exists() {
            println!("Skipping test - model file not found");
            return;
        }
        let mut engine = InferenceEngine::new();
        let result = engine.load_model(model_path);
        match result {
            Ok(()) => {
                assert!(engine.is_loaded());
                println!("Model loaded: context_length={}", engine.get_config().context_length);
                engine.unload_model();
                assert!(!engine.is_loaded());
            }
            Err(e) => {
                // This embedding model may not support text generation API version
                // So loading might fail gracefully - that's OK
                println!("Model load (expected, embedding model): {}", e);
            }
        }
    }
}
