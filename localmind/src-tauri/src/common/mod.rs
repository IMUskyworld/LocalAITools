// 公共模块 — 错误类型、配置、常量

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 统一 IPC 响应结构
#[derive(Debug, Serialize, Deserialize)]
pub struct AppResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

impl<T: Serialize> AppResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            error_code: None,
        }
    }

    pub fn err(code: &str, msg: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.to_string()),
            error_code: Some(code.to_string()),
        }
    }
}

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub gateway: GatewayConfig,
    pub model: ModelConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub wss_url: String,
    pub api_base: String,
    pub model_display_name: String,
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub default_windows: ModelItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelItem {
    pub name: String,
    pub size_mb: u64,
    pub sha256: String,
    pub download_url: String,
    pub license: String,
    pub min_ram_mb: u64,
}

impl AppConfig {
    pub fn load() -> Self {
        // 从配置文件加载，实际项目中从固定路径读取
        AppConfig {
            gateway: GatewayConfig {
                wss_url: "wss://relay.example.com/ws".to_string(),
                api_base: "http://localhost:3000".to_string(),
                model_display_name: "Deepseek-V4-Pro".to_string(),
                api_key: String::new(),  // 已移除遗留 gateway key（死配置，未使用）
            },
            model: ModelConfig {
                default_windows: ModelItem {
                    name: "DeepSeek-R1-0528-Qwen3-8B Q4_K_M".to_string(),
                    size_mb: 5325,
                    sha256: String::new(),
                    download_url: String::new(),
                    license: "MIT".to_string(),
                    min_ram_mb: 8192,
                },
            },
        }
    }
}
