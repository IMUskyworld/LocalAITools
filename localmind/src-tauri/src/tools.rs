// 电脑操作工具模块（Phase 0 收缩版）
// 仅保留当前 UI 真正使用的 open_app / get_common_paths / check_ollama。
// 任意命令执行、删除、写入、移动等 Legacy Tauri Command 已移除；
// Python Agent 的同名工具在独立 Tool Guard 之下运行，不复用这里的实现。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common::AppResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: String,
}

impl ToolResult {
    fn ok(output: impl Into<String>) -> AppResponse<ToolResult> {
        AppResponse::ok(ToolResult { success: true, output: output.into(), error: String::new() })
    }

    fn err(msg: impl Into<String>) -> AppResponse<ToolResult> {
        AppResponse::ok(ToolResult { success: false, output: String::new(), error: msg.into() })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum OpenTarget {
    Url(String),
    Path(PathBuf),
    App(PathBuf),
}

fn user_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

fn allowed_open_roots() -> Vec<PathBuf> {
    let Some(home) = user_home() else {
        return Vec::new();
    };
    [
        home.join("Desktop"),
        home.join("Documents"),
        home.join("Downloads"),
        home.join("OneDrive").join("Desktop"),
        home.join("OneDrive").join("Documents"),
    ]
    .into_iter()
    .filter(|p| p.exists())
    .collect()
}

fn windows_root() -> Option<PathBuf> {
    std::env::var_os("WINDIR").map(PathBuf::from)
}

fn reject_unsafe_path_text(raw: &str) -> Result<(), String> {
    if raw.contains('\0') {
        return Err("路径包含非法字符".to_string());
    }
    let normalized = raw.replace('/', "\\");
    let lower = normalized.to_ascii_lowercase();
    if normalized.starts_with("\\\\") || normalized.starts_with("//") {
        return Err("不允许 UNC 或 device namespace 路径".to_string());
    }
    if lower == ".." || lower.starts_with("..\\") || lower.contains("\\..\\") || lower.ends_with("\\..") {
        return Err("路径不得包含 ..".to_string());
    }
    if lower.starts_with("c:\\windows")
        || lower.starts_with("c:\\program files")
        || lower.starts_with("c:\\program files (x86)")
    {
        return Err("不允许打开系统目录或 Program Files".to_string());
    }
    Ok(())
}

fn is_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    roots.iter().any(|root| {
        root.canonicalize()
            .map(|root| path == root || path.starts_with(root))
            .unwrap_or(false)
    })
}

fn is_executable_like(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "exe" | "com" | "bat" | "cmd" | "ps1" | "vbs" | "js" | "jse" | "wsf" | "wsh" | "msi" | "lnk" | "reg"
    )
}

fn validate_open_target_with_roots(target: &str, roots: &[PathBuf]) -> Result<OpenTarget, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("打开目标为空".to_string());
    }

    if target.starts_with("http://") || target.starts_with("https://") {
        let url = url::Url::parse(target).map_err(|e| format!("URL 无效: {}", e))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("只允许打开 http/https URL".to_string());
        }
        return Ok(OpenTarget::Url(url.to_string()));
    }

    let app = target.to_ascii_lowercase();
    let system_root = windows_root().unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let app_whitelist = [
        ("notepad", system_root.join("System32").join("notepad.exe")),
        ("notepad.exe", system_root.join("System32").join("notepad.exe")),
        ("calc", system_root.join("System32").join("calc.exe")),
        ("calc.exe", system_root.join("System32").join("calc.exe")),
        ("mspaint", system_root.join("System32").join("mspaint.exe")),
        ("mspaint.exe", system_root.join("System32").join("mspaint.exe")),
        ("explorer", system_root.join("explorer.exe")),
        ("explorer.exe", system_root.join("explorer.exe")),
    ];
    if let Some((_, exe)) = app_whitelist.iter().find(|(name, _)| *name == app) {
        if !exe.exists() {
            return Err(format!("白名单应用不存在: {}", exe.display()));
        }
        return Ok(OpenTarget::App(exe.clone()));
    }

    reject_unsafe_path_text(target)?;
    let path = PathBuf::from(target);
    if !path.exists() {
        return Err("目标不是受控白名单应用，路径也不存在".to_string());
    }
    if !path.is_dir() && is_executable_like(&path) {
        return Err("不允许通过 open_app 启动任意可执行文件".to_string());
    }
    if !is_within_roots(&path, roots) {
        return Err("路径不在允许打开的用户目录范围内".to_string());
    }
    Ok(OpenTarget::Path(path))
}

fn spawn_open_target(target: OpenTarget) -> Result<(), String> {
    match target {
        OpenTarget::Url(value) => {
            let explorer = windows_root()
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
                .join("explorer.exe");
            Command::new(explorer)
                .arg(value)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("打开失败: {}", e))
        }
        OpenTarget::Path(value) => {
            let explorer = windows_root()
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
                .join("explorer.exe");
            Command::new(explorer)
                .arg(value)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("打开失败: {}", e))
        }
        OpenTarget::App(exe) => Command::new(exe)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("启动应用失败: {}", e)),
    }
}

// ========== 1. 打开应用/文件/URL（限制版） ==========

#[tauri::command]
pub async fn open_app(target: String) -> Result<AppResponse<ToolResult>, String> {
    let roots = allowed_open_roots();
    match validate_open_target_with_roots(&target, &roots).and_then(spawn_open_target) {
        Ok(()) => Ok(ToolResult::ok(format!("已打开: {}", target))),
        Err(e) => Ok(ToolResult::err(e)),
    }
}

// ========== 2. Ollama 检测 ==========

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

// ========== 3. 获取常见路径 ==========

#[tauri::command]
pub async fn get_common_paths() -> Result<AppResponse<Vec<String>>, String> {
    let home = user_home().unwrap_or_default();
    let desktop = {
        let onedrive = home.join("OneDrive").join("Desktop");
        if onedrive.exists() {
            onedrive
        } else {
            home.join("Desktop")
        }
    };
    let docs = {
        let onedrive = home.join("OneDrive").join("Documents");
        if onedrive.exists() {
            onedrive
        } else {
            home.join("Documents")
        }
    };
    let downloads = home.join("Downloads");
    let cwd = std::env::current_dir().unwrap_or_default();
    Ok(AppResponse::ok(vec![
        format!("用户目录: {}", home.to_string_lossy()),
        format!("桌面: {}", desktop.to_string_lossy()),
        format!("文档: {}", docs.to_string_lossy()),
        format!("下载: {}", downloads.to_string_lossy()),
        format!("当前目录: {}", cwd.to_string_lossy()),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unc_and_device_paths() {
        assert!(reject_unsafe_path_text(r"\\server\share").is_err());
        assert!(reject_unsafe_path_text(r"\\?\C:\Windows").is_err());
        assert!(reject_unsafe_path_text(r"C:\Windows\System32").is_err());
    }

    #[test]
    fn allows_http_urls() {
        let result = validate_open_target_with_roots("https://example.com", &[]).unwrap();
        assert!(matches!(result, OpenTarget::Url(_)));
    }

    #[test]
    fn rejects_arbitrary_executables() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("bad.exe");
        std::fs::write(&exe, b"not really an exe").unwrap();
        let roots = vec![temp.path().to_path_buf()];
        assert!(validate_open_target_with_roots(exe.to_str().unwrap(), &roots).is_err());
    }

    #[test]
    fn allows_regular_file_under_root() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("notes.txt");
        std::fs::write(&file, b"hello").unwrap();
        let roots = vec![temp.path().to_path_buf()];
        let result = validate_open_target_with_roots(file.to_str().unwrap(), &roots).unwrap();
        assert!(matches!(result, OpenTarget::Path(_)));
    }

    #[test]
    fn rejects_path_traversal_text() {
        assert!(reject_unsafe_path_text(r"C:\Users\me\..\Windows").is_err());
        assert!(reject_unsafe_path_text(r"..\Desktop\escape.txt").is_err());
    }
}
