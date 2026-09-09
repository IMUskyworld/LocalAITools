// 电脑操作工具模块 — 让 AI 具备操作 Windows 的能力
// 提供：文件读写/目录浏览/执行命令/打开应用 等工具，供前端 agent 循环调用

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::AppResponse;

// ========== 工具结果统一结构 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,   // 文本输出（命令 stdout、文件内容等）
    pub error: String,    // 错误信息（success=false 时）
}

impl ToolResult {
    fn ok(output: impl Into<String>) -> AppResponse<ToolResult> {
        AppResponse::ok(ToolResult { success: true, output: output.into(), error: String::new() })
    }
    fn err(msg: impl Into<String>) -> AppResponse<ToolResult> {
        AppResponse::ok(ToolResult { success: false, output: String::new(), error: msg.into() })
    }
}

/// 输出最大长度（防刷屏）
const MAX_OUTPUT_CHARS: usize = 6000;

fn truncate_output(s: &str) -> String {
    if s.chars().count() > MAX_OUTPUT_CHARS {
        let t: String = s.chars().take(MAX_OUTPUT_CHARS).collect();
        format!("{}…\n[输出已截断，剩余 {} 字符]", t, s.chars().count() - MAX_OUTPUT_CHARS)
    } else {
        s.to_string()
    }
}

// ========== 1. 写文件（新建/覆盖） ==========

#[tauri::command]
pub async fn write_file(path: String, content: String) -> Result<AppResponse<ToolResult>, String> {
    let p = PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }
    fs::write(&p, &content).map_err(|e| format!("写入文件失败: {}", e))?;
    Ok(ToolResult::ok(format!("已写入文件: {}\n大小: {} 字节", path, content.len())))
}

// ========== 2. 追加写文件 ==========

#[tauri::command]
pub async fn append_file(path: String, content: String) -> Result<AppResponse<ToolResult>, String> {
    let p = PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
        .map_err(|e| format!("打开文件失败: {}", e))?;
    f.write_all(content.as_bytes()).map_err(|e| format!("追加内容失败: {}", e))?;
    Ok(ToolResult::ok(format!("已追加内容到: {}\n当前文件大小: {} 字节", path, fs::metadata(&p).map(|m| m.len()).unwrap_or(0))))
}

// ========== 3. 读文件 ==========

#[tauri::command]
pub async fn read_file(path: String) -> Result<AppResponse<ToolResult>, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Ok(ToolResult::err(format!("文件不存在: {}", path)));
    }
    let bytes = fs::read(&p).map_err(|e| format!("读取文件失败: {}", e))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let size = bytes.len();
    Ok(ToolResult::ok(format!("【文件: {} | 大小: {} 字节】\n\n{}", path, size, truncate_output(&text))))
}

// ========== 4. 列出目录 ==========

#[tauri::command]
pub async fn list_dir(path: String) -> Result<AppResponse<ToolResult>, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Ok(ToolResult::err(format!("目录不存在: {}", path)));
    }
    let entries = fs::read_dir(&p).map_err(|e| format!("读取目录失败: {}", e))?;
    let mut lines = vec![format!("【目录: {}】", path)];
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() {
            dirs.push(format!("[目录] {}", name));
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            files.push(format!("[文件] {} ({} 字节)", name, size));
        }
    }
    dirs.sort();
    files.sort();
    lines.extend(dirs);
    lines.extend(files);
    if lines.len() == 1 {
        lines.push("（空目录）".to_string());
    }
    Ok(ToolResult::ok(lines.join("\n")))
}

// ========== 5. 删除文件/目录 ==========

#[tauri::command]
pub async fn delete_path(path: String) -> Result<AppResponse<ToolResult>, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Ok(ToolResult::err(format!("路径不存在: {}", path)));
    }
    if p.is_dir() {
        fs::remove_dir_all(&p).map_err(|e| format!("删除目录失败: {}", e))?;
        Ok(ToolResult::ok(format!("已删除目录: {}", path)))
    } else {
        fs::remove_file(&p).map_err(|e| format!("删除文件失败: {}", e))?;
        Ok(ToolResult::ok(format!("已删除文件: {}", path)))
    }
}

// ========== 6. 重命名/移动 ==========

#[tauri::command]
pub async fn rename_path(from: String, to: String) -> Result<AppResponse<ToolResult>, String> {
    let f = PathBuf::from(&from);
    let t = PathBuf::from(&to);
    if !f.exists() {
        return Ok(ToolResult::err(format!("源路径不存在: {}", from)));
    }
    if let Some(parent) = t.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败: {}", e))?;
        }
    }
    fs::rename(&f, &t).map_err(|e| format!("重命名/移动失败: {}", e))?;
    Ok(ToolResult::ok(format!("已移动/重命名: {} → {}", from, to)))
}

// ========== 7. 执行终端命令 ==========

#[tauri::command]
pub async fn run_command(command: String) -> Result<AppResponse<ToolResult>, String> {
    let output = run_shell_command(&command)?;
    Ok(ToolResult::ok(output))
}

/// 在 Windows 上执行命令（用 PowerShell，兼容 CMD）
fn run_shell_command(command: &str) -> Result<String, String> {
    // 优先 PowerShell（功能更强，输出格式统一）
    let cmd_result = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &format!("& {{ {} }}", command)])
        .output();

    match cmd_result {
        Ok(out) => {
            let mut text = String::new();
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            if !stdout.is_empty() {
                text.push_str(&stdout);
            }
            if !stderr.is_empty() {
                text.push_str(&format!("\n[错误输出] {}", stderr));
            }
            let status = out.status.code().unwrap_or(-1);
            if text.trim().is_empty() {
                text = format!("（命令执行完成，无输出，退出码 {status}）");
            }
            Ok(format!("$ {}\n{}", command, truncate_output(&text)))
        }
        Err(e) => Err(format!("执行命令失败: {}", e)),
    }
}

// ========== 8. 打开应用/程序/文件 ==========

#[tauri::command]
pub async fn open_app(target: String) -> Result<AppResponse<ToolResult>, String> {
    // 尝试直接作为路径/命令打开；否则尝试 Start-Process
    let p = PathBuf::from(&target);
    let exists = p.exists();

    let result = if exists {
        // 用 shell 打开文件/文件夹（关联程序）
        let _ = std::process::Command::new("cmd.exe")
            .args(["/C", "start", "", &target])
            .spawn();
        Ok("已打开".to_string())
    } else {
        // 尝试 Start-Process
        let cmd = format!("Start-Process '{}' -ErrorAction Stop", target.replace('\'', "''"));
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
            .output();
        match output {
            Ok(out) if out.status.success() => Ok(format!("已启动: {}", target)),
            Ok(out) => Err(format!(
                "启动失败: {}",
                String::from_utf8_lossy(&out.stderr).to_string()
            )),
            Err(e) => Err(format!("启动失败: {}", e)),
        }
    };

    match result {
        Ok(msg) => Ok(ToolResult::ok(msg)),
        Err(e) => Ok(ToolResult::err(e)),
    }
}

// ========== 9. Ollama 检测（走 Rust 端，避免 WebView2 访问 localhost 的限制） ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaStatusDto {
    pub running: bool,
    pub models: Vec<OllamaModelInfo>,
    pub error: String,
}

#[tauri::command]
pub async fn check_ollama() -> Result<AppResponse<OllamaStatusDto>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| format!("创建客户端失败: {}", e))?;

    let resp = match client.get("http://localhost:11434/api/tags").send().await {
        Ok(r) => r,
        Err(_) => {
            return Ok(AppResponse::ok(OllamaStatusDto {
                running: false,
                models: Vec::new(),
                error: "无法连接 Ollama".to_string(),
            }));
        }
    };

    if !resp.status().is_success() {
        return Ok(AppResponse::ok(OllamaStatusDto {
            running: false,
            models: Vec::new(),
            error: format!("Ollama 返回 {}", resp.status()),
        }));
    }

    let data: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return Ok(AppResponse::ok(OllamaStatusDto {
                running: true,
                models: Vec::new(),
                error: format!("解析失败: {}", e),
            }));
        }
    };

    let models: Vec<OllamaModelInfo> = data
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .map(|m| OllamaModelInfo {
                    name: m.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
                    size: m.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
                    modified_at: m.get("modified_at").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(AppResponse::ok(OllamaStatusDto {
        running: true,
        models,
        error: String::new(),
    }))
}

// ========== 10. Ollama 对话代理（走 Rust 端 reqwest，绕开 WebView2 localhost 限制） ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaChatResponse {
    pub success: bool,
    pub content: String,
    pub tool_calls: serde_json::Value,
    pub error: String,
}

#[tauri::command]
pub async fn ollama_chat(
    model: String,
    messages: serde_json::Value,
    tools: Option<serde_json::Value>,
) -> Result<AppResponse<OllamaChatResponse>, String> {
    let tools = tools.unwrap_or(serde_json::Value::Null);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建客户端失败: {}", e))?;

    // 规范化消息：Ollama 原生 /api/chat 要求 tool 消息不能带 name/tool_call_id，
    // 且 assistant 的 tool_calls.arguments 必须是对象（非字符串）
    let normalized_messages = normalize_ollama_messages(&messages);

    // 组装 Ollama 原生 /api/chat 请求体
    let mut body = serde_json::json!({
        "model": model,
        "messages": normalized_messages,
        "stream": false,
        "options": { "temperature": 0.7 }
    });
    if !tools.is_null() && tools.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
        body["tools"] = tools;
    }

    let resp = match client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Ok(AppResponse::ok(OllamaChatResponse {
                success: false,
                content: String::new(),
                tool_calls: serde_json::Value::Null,
                error: format!("无法连接 Ollama: {}", e),
            }));
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Ok(AppResponse::ok(OllamaChatResponse {
            success: false,
            content: String::new(),
            tool_calls: serde_json::Value::Null,
            error: format!("Ollama 返回 {}: {}", status, text),
        }));
    }

    let data: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return Ok(AppResponse::ok(OllamaChatResponse {
                success: false,
                content: String::new(),
                tool_calls: serde_json::Value::Null,
                error: format!("解析 Ollama 响应失败: {}", e),
            }));
        }
    };

    let msg = data.get("message").cloned().unwrap_or(serde_json::Value::Null);
    let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
    let tool_calls = msg.get("tool_calls").cloned().unwrap_or(serde_json::Value::Null);

    Ok(AppResponse::ok(OllamaChatResponse {
        success: true,
        content,
        tool_calls,
        error: String::new(),
    }))
}

/// 规范化消息为 Ollama 原生 /api/chat 兼容格式
fn normalize_ollama_messages(messages: &serde_json::Value) -> serde_json::Value {
    let arr = match messages.as_array() {
        Some(a) => a,
        None => return messages.clone(),
    };

    let normalized: Vec<serde_json::Value> = arr
        .iter()
        .map(|m| {
            let mut msg = m.clone();
            if let Some(obj) = msg.as_object_mut() {
                let role = obj.get("role").and_then(|r| r.as_str()).unwrap_or("");
                match role {
                    // tool 消息：Ollama 不允许 name/tool_call_id 字段，剥掉
                    "tool" => {
                        obj.remove("name");
                        obj.remove("tool_call_id");
                    }
                    // assistant 消息：tool_calls.arguments 转对象
                    "assistant" => {
                        if let Some(tcs) = obj.get_mut("tool_calls").and_then(|t| t.as_array_mut()) {
                            for tc in tcs.iter_mut() {
                                if let Some(fn_obj) = tc.get_mut("function") {
                                    if let Some(args) = fn_obj.get_mut("arguments") {
                                        if let Some(s) = args.as_str() {
                                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                                                *args = parsed;
                                            }
                                        }
                                    }
                                    // 剥掉 function 里的 index 等多余字段（保留 name + arguments）
                                    if let Some(fobj) = fn_obj.as_object_mut() {
                                        fobj.remove("index");
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            msg
        })
        .collect();

    serde_json::Value::Array(normalized)
}

// ========== 11. 生成 PPT 工具 ==========

/// 生成文档：支持 ppt/docx/xlsx/pdf 四类
/// 调用随 exe 分发的 make_doc.exe（PyInstaller 打包，不依赖本机 Python）
/// 注意：所有错误都以 Ok(ToolResult::err) 返回，绝不返回 Err，
/// 否则 Tauri 会把 Err 变成 invoke 异常，前端只能看到笼统的"工具返回异常"。
/// rename_all = "snake_case"：前端 AI 生成的 tool call 用 doc_type（snake_case），
/// 而 Tauri 默认期望 docType（camelCase），不转换会导致 doc_type 收不到变成空。
#[tauri::command(rename_all = "snake_case")]
pub async fn create_doc(
    doc_type: Option<String>,
    spec: Option<serde_json::Value>,
) -> Result<AppResponse<ToolResult>, String> {
    let doc_type = doc_type.unwrap_or_default();
    let spec = spec.unwrap_or(serde_json::Value::Null);

    // 合法类型校验
    let is_valid = match doc_type.as_str() {
        "ppt" | "pptx" | "docx" | "doc" | "word" | "xlsx" | "xls" | "excel" | "pdf" => true,
        _ => false,
    };
    if !is_valid {
        return Ok(ToolResult::err(format!(
            "不支持的文档类型: {}（支持 ppt/docx/xlsx/pdf）",
            doc_type
        )));
    }

    let doc_exe = find_doc_exe();
    if doc_exe.is_empty() {
        return Ok(ToolResult::err(
            "未找到 make_doc.exe，无法生成文档。请确认它与 LocalMind.exe 在同一目录（或 scripts/ 子目录）。",
        ));
    }

    // 把 spec 里所有反斜杠统一为正斜杠（Windows 兼容），
    // 避免路径含 "\U" 等被 JSON 视为无效转义，导致 make_doc.exe 解析失败
    let normalized_spec = normalize_path_separators(&spec);

    // 写 spec 到临时文件（失败时返回具体错误，不抛 Err）
    let spec_json = match serde_json::to_string(&normalized_spec) {
        Ok(s) => s,
        Err(e) => return Ok(ToolResult::err(format!("spec 序列化失败: {}", e))),
    };
    let tmp_spec = std::env::temp_dir().join(format!("localmind_{}_spec.json", doc_type));
    if let Err(e) = fs::write(&tmp_spec, &spec_json) {
        return Ok(ToolResult::err(format!("写临时文件失败: {}", e)));
    }

    // 调用 make_doc.exe --type <doc_type> <spec.json>
    let output = std::process::Command::new(&doc_exe)
        .arg("--type")
        .arg(&doc_type)
        .arg(tmp_spec.to_str().unwrap_or(""))
        .output();

    let _ = fs::remove_file(&tmp_spec);

    match output {
        Ok(out) => {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                let path = spec.get("path").and_then(|p| p.as_str()).unwrap_or("");
                Ok(ToolResult::ok(format!("已生成文档: {}\n{}", path, text.trim())))
            } else {
                let err = String::from_utf8_lossy(&out.stderr).to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                Ok(ToolResult::err(format!("生成失败: {}\n{}", err, stdout)))
            }
        }
        Err(e) => Ok(ToolResult::err(format!("执行 make_doc.exe 失败: {}", e))),
    }
}

/// 递归把 spec JSON 中所有字符串的反斜杠 `\` 替换为正斜杠 `/`。
/// Windows 路径对正斜杠完全兼容（C:/Users/...），但反斜杠在 JSON 里
/// 需要转义成 `\\`，一旦出现 `\U`、`\w` 这类组合就成了非法转义。
fn normalize_path_separators(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(s.replace('\\', "/")),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(normalize_path_separators).collect())
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), normalize_path_separators(v));
            }
            serde_json::Value::Object(out)
        }
        other => other.clone(),
    }
}

/// 查找 make_doc.exe（随 exe 分发，无需 Python）
fn find_doc_exe() -> String {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let cwd = std::env::current_dir().ok();

    let mut candidates: Vec<String> = Vec::new();
    // 1. exe 同目录（打包后最可靠）
    if let Some(d) = &exe_dir {
        candidates.push(d.join("make_doc.exe").to_string_lossy().to_string());
        candidates.push(d.join("scripts").join("make_doc.exe").to_string_lossy().to_string());
    }
    // 2. 开发模式：项目目录 src-tauri/scripts
    if let Some(d) = &cwd {
        candidates.push(d.join("src-tauri/scripts/dist/make_doc.exe").to_string_lossy().to_string());
        candidates.push(d.join("src-tauri/scripts").join("make_doc.exe").to_string_lossy().to_string());
    }
    for c in candidates {
        if Path::new(&c).exists() {
            return c;
        }
    }
    String::new()
}

// ========== 12. 获取工作目录/常见路径 ==========

#[tauri::command]
pub async fn get_common_paths() -> Result<AppResponse<Vec<String>>, String> {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    // 桌面优先 OneDrive 路径（用户桌面在 OneDrive 同步目录下）
    let desktop = {
        let onedrive = Path::new(&home).join("OneDrive").join("Desktop");
        if onedrive.exists() {
            onedrive.to_string_lossy().to_string()
        } else {
            Path::new(&home).join("Desktop").to_string_lossy().to_string()
        }
    };
    let docs = std::env::var("USERPROFILE")
        .map(|h| Path::new(&h).join("Documents").to_string_lossy().to_string())
        .unwrap_or_default();
    let downloads = std::env::var("USERPROFILE")
        .map(|h| Path::new(&h).join("Downloads").to_string_lossy().to_string())
        .unwrap_or_default();
    let cwd = std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    Ok(AppResponse::ok(vec![
        format!("用户目录: {}", home),
        format!("桌面: {}", desktop),
        format!("文档: {}", docs),
        format!("下载: {}", downloads),
        format!("当前目录: {}", cwd),
    ]))
}
