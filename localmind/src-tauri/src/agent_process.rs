// Python Agent 服务进程管理（Pydantic AI Harness）
// 懒启动：首次 get_agent_config 时 spawn Python Agent 服务（dev 用 python 脚本，prod 用打包的
// localmind-agent.exe），从 stdout 读端口，暴露给前端 fetch SSE；应用退出时 kill 子进程。

use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::common::AppResponse;

/// 暴露给前端的 Agent 连接信息
#[derive(Debug, Clone, Serialize)]
pub struct AgentConfig {
    pub port: u16,
    pub token: String,
}

/// 正在运行的 Agent 进程句柄
pub struct AgentHandle {
    pub port: u16,
    pub token: String,
    pub child: Option<Child>,
}

/// Agent 进程管理器（懒启动 + 缓存 + 退出清理）
pub struct AgentManager {
    handle: Mutex<Option<AgentHandle>>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self { handle: Mutex::new(None) }
    }

    /// 获取 Agent 连接信息；未启动则先启动（带缓存）。
    /// 阻塞式：首次启动含 Python 导入 + 端口握手，约 1-3 秒，调用方应走 spawn_blocking。
    pub fn get_or_spawn(&self) -> Result<AgentConfig, String> {
        if let Some(h) = self.handle.lock().unwrap().as_ref() {
            return Ok(AgentConfig { port: h.port, token: h.token.clone() });
        }
        let handle = spawn_agent()?;
        let config = AgentConfig { port: handle.port, token: handle.token.clone() };
        *self.handle.lock().unwrap() = Some(handle);
        Ok(config)
    }

    /// 应用退出时杀掉 Agent 子进程
    pub fn kill(&self) {
        if let Some(mut h) = self.handle.lock().unwrap().take() {
            if let Some(mut child) = h.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[tauri::command]
pub async fn get_agent_config(
    state: tauri::State<'_, crate::AppState>,
) -> Result<AppResponse<AgentConfig>, String> {
    let mgr = state.agent_manager.clone();
    let config = tauri::async_runtime::spawn_blocking(move || mgr.get_or_spawn())
        .await
        .map_err(|e| format!("Agent 启动任务失败: {}", e))??;
    Ok(AppResponse::ok(config))
}

// ========== 启动 ==========

/// DeepSeek API key（在线模式）。优先读运行时环境变量 LOCALMIND_DEEPSEEK_KEY，
/// 通过子进程环境变量传给 Python Agent 服务，绝不进入前端 JS bundle 或 HTTP 请求体。
/// 未设置运行时环境变量时，回退到编译期烘焙的默认 key（option_env!("LOCALMIND_DEEPSEEK_KEY")），
/// 这样打包出的 exe 开箱即用，用户无需手动配置环境变量。
/// 代码仓库不包含任何真实 key（编译期值由构建环境注入），可安全提交到 GitHub。
fn deepseek_api_key() -> String {
    let runtime = std::env::var("LOCALMIND_DEEPSEEK_KEY").unwrap_or_default();
    if !runtime.is_empty() {
        return runtime;
    }
    option_env!("LOCALMIND_DEEPSEEK_KEY").unwrap_or("").to_string()
}

/// 生成随机会话 token（Python 端校验请求头，防止本地任意进程触发写文件/生成文档）
fn gen_token() -> String {
    use rand::Rng;
    let bytes: Vec<u8> = (0..24).map(|_| rand::thread_rng().gen::<u8>()).collect();
    hex::encode(bytes)
}

/// 定位 Agent 启动方式：
/// - debug（开发）优先 python scripts/agent_server.py——改 Python 代码即时生效，不用重新打包
/// - release（分发）用打包的 localmind-agent.exe
fn locate_launcher() -> Result<(PathBuf, Vec<String>, PathBuf), String> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let script = project.join("scripts/agent_server.py");

    if cfg!(debug_assertions) {
        // 开发：python 脚本优先（改代码即生效）
        if script.exists() {
            return Ok((
                PathBuf::from("python"),
                vec![script.to_string_lossy().to_string()],
                project,
            ));
        }
    }
    // 分发：打包的 onedir exe
    if let Some(exe) = find_agent_exe() {
        let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        return Ok((exe, Vec::new(), dir));
    }
    // release 兜底：仍允许 python 脚本
    if script.exists() {
        return Ok((
            PathBuf::from("python"),
            vec![script.to_string_lossy().to_string()],
            project,
        ));
    }
    Err("未找到 Agent 服务（localmind-agent.exe 或 scripts/agent_server.py）".to_string())
}

/// 查找打包的 localmind-agent.exe（onedir 目录，仿 tools.rs::find_doc_exe 的候选路径）
fn find_agent_exe() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let cwd = std::env::current_dir().ok();

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(d) = &exe_dir {
        // 安装版：exe 与 localmind-agent/ 同级
        candidates.push(d.join("localmind-agent/localmind-agent.exe"));
        candidates.push(d.join("scripts/localmind-agent/localmind-agent.exe"));
        // standalone：LocalMind.exe 与 LocalMindScripts/ 同级
        candidates.push(d.join("LocalMindScripts/localmind-agent/localmind-agent.exe"));
    }
    if let Some(d) = &cwd {
        candidates.push(d.join("LocalMindScripts/localmind-agent/localmind-agent.exe"));
        candidates.push(d.join("scripts/dist/localmind-agent/localmind-agent.exe"));
    }
    candidates.into_iter().find(|p| p.exists())
}

/// 读取 %TEMP%/localmind_agent_port_{pid}.txt（stdout 通道的兜底）
fn read_port_file(pid: u32) -> Option<u16> {
    let path = std::env::temp_dir().join(format!("localmind_agent_port_{}.txt", pid));
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
}

fn spawn_agent() -> Result<AgentHandle, String> {
    let (cmd, args, cwd) = locate_launcher()?;
    let token = gen_token();

    let mut cmd_builder = Command::new(&cmd);
    cmd_builder
        .args(&args)
        .current_dir(&cwd)
        .env("LOCALMIND_AGENT_TOKEN", &token)
        // DeepSeek key 只经环境变量传给 Python（不进前端/HTTP）
        .env("LOCALMIND_DEEPSEEK_KEY", deepseek_api_key())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd_builder.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd_builder
        .spawn()
        .map_err(|e| format!("启动 Agent 服务失败（{}）: {}", cmd.display(), e))?;
    let pid = child.id();

    // 后台线程读子进程 stdout 的 PORT= 行
    let stdout = child.stdout.take().ok_or("无法捕获 Agent 输出")?;
    let port_slot: Arc<Mutex<Option<u16>>> = Arc::new(Mutex::new(None));
    let slot2 = port_slot.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if let Some(rest) = line.strip_prefix("PORT=") {
                if let Ok(p) = rest.trim().parse::<u16>() {
                    *slot2.lock().unwrap() = Some(p);
                    break;
                }
            }
        }
    });

    // 带超时轮询端口
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut port = None;
    while Instant::now() < deadline {
        if let Some(p) = *port_slot.lock().unwrap() {
            port = Some(p);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // stdout 兜底：读端口文件
    if port.is_none() {
        port = read_port_file(pid);
    }

    let port = port.ok_or_else(|| {
        let _ = child.kill();
        "Agent 服务启动超时（未收到端口）".to_string()
    })?;

    Ok(AgentHandle { port, token, child: Some(child) })
}
