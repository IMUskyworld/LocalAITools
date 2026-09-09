// 文件处理模块 — Windows 端文件读取能力
// 供前端选文件后读取文本内容，供 AI 做总结/翻译/问答等

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::common::AppResponse;

/// 读取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadResult {
    pub path: String,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub content: String,
    pub truncated: bool,
}

/// 支持直接读取的文本类扩展名
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "json", "jsonl", "csv", "tsv", "log",
    "yml", "yaml", "toml", "xml", "html", "htm", "css", "js", "ts",
    "jsx", "tsx", "py", "rs", "go", "java", "c", "cpp", "h", "hpp",
    "rb", "php", "sql", "sh", "bat", "ps1", "ini", "cfg", "conf",
    "env", "gitignore", "dockerfile", "makefile",
];

/// 最大读取字节数（保护内存，超出截断）
const MAX_READ_BYTES: usize = 100_000; // ~100KB

/// 读取文本文件内容
#[tauri::command]
pub async fn read_text_file(path: String) -> Result<AppResponse<FileReadResult>, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Ok(AppResponse::err("300001", "文件不存在"));
    }

    let metadata = fs::metadata(p).map_err(|e| format!("读取文件信息失败: {}", e))?;
    if !metadata.is_file() {
        return Ok(AppResponse::err("300002", "不是有效的文件"));
    }
    let size_bytes = metadata.len();

    let extension = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // 判断是否为支持读取的文本类型
    let is_text = TEXT_EXTENSIONS.contains(&extension.as_str())
        || extension.is_empty()
        || is_probably_text(&p, size_bytes);

    if !is_text {
        return Ok(AppResponse::ok(FileReadResult {
            path: path.clone(),
            file_name: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            extension,
            size_bytes,
            content: String::new(),
            truncated: false,
        }));
    }

    // 读取内容（带大小限制）
    let mut bytes = Vec::new();
    let truncated = {
        use std::io::Read;
        let mut file = fs::File::open(&p).map_err(|e| format!("打开文件失败: {}", e))?;
        let mut limited = file.by_ref().take(MAX_READ_BYTES as u64 + 1);
        limited.read_to_end(&mut bytes).map_err(|e| format!("读取文件失败: {}", e))?;
        bytes.len() > MAX_READ_BYTES
    };
    if truncated {
        bytes.truncate(MAX_READ_BYTES);
    }

    // 尝试 UTF-8 解码，失败则尝试 GBK 等
    let content = match String::from_utf8(bytes.clone()) {
        Ok(s) => s,
        Err(_) => {
            // 尝试用宽松方式解析（跳过非法字节）
            String::from_utf8_lossy(&bytes).to_string()
        }
    };

    Ok(AppResponse::ok(FileReadResult {
        path: path.clone(),
        file_name: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        extension,
        size_bytes,
        content,
        truncated,
    }))
}

/// 粗略判断是否可能是文本文件（无扩展名时，检查前若干字节是否含 NUL）
fn is_probably_text(p: &Path, size: u64) -> bool {
    if size == 0 {
        return true;
    }
    let mut buf = [0u8; 256];
    if let Ok(mut f) = fs::File::open(p) {
        use std::io::Read;
        let n = f.read(&mut buf).unwrap_or(0);
        if n == 0 {
            return true;
        }
        // 前 256 字节不含 NUL 大概率是文本
        !buf[..n].contains(&0u8)
    } else {
        false
    }
}
