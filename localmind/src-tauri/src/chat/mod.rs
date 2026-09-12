// 对话引擎模块 — 只保留当前 UI 使用的模式切换和停止标记。
// Session / Message / Turn 的持久化统一由 chat_api + SQLite 处理。

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::common::AppResponse;
use crate::storage::StorageManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub mode: String,
    pub message_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub model_label: String,
    /// 本轮消耗的 token 总数（来自 API 的 usage，精确值；旧数据可能为空）
    pub token_count: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMode {
    pub mode: String,
    pub gateway_health: String,
    pub inference_state: String,
    pub current_model_label: String,
}

pub struct ChatEngine {
    current_mode: RwLock<String>,
    is_streaming: RwLock<bool>,
}

impl ChatEngine {
    pub fn new(_storage: Arc<RwLock<StorageManager>>) -> Self {
        Self {
            current_mode: RwLock::new("online".to_string()),
            is_streaming: RwLock::new(false),
        }
    }

    pub async fn get_current_mode(&self) -> String {
        self.current_mode.read().await.clone()
    }

    pub async fn set_mode(&self, mode: &str) -> Result<String, String> {
        if mode != "online" && mode != "offline" {
            return Err("Invalid mode".to_string());
        }
        *self.current_mode.write().await = mode.to_string();
        Ok(mode.to_string())
    }
}

#[tauri::command]
pub async fn stop_generation(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<bool>, String> {
    let engine = state.chat_engine.read().await;
    *engine.is_streaming.write().await = false;
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn switch_mode(
    mode: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<ChatMode>, String> {
    let engine = state.chat_engine.read().await;
    let current_mode = engine.set_mode(&mode).await?;
    let model_label = if current_mode == "offline" {
        "本地模型".to_string()
    } else {
        "DeepSeek V4 Flash".to_string()
    };
    let inference_state = if current_mode == "offline" {
        state.model_manager.read().await.get_state().await
    } else {
        "running".to_string()
    };

    Ok(AppResponse::ok(ChatMode {
        mode: current_mode,
        gateway_health: "ok".to_string(),
        inference_state,
        current_model_label: model_label,
    }))
}

#[tauri::command]
pub async fn get_mode(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<ChatMode>, String> {
    let engine = state.chat_engine.read().await;
    let current_mode = engine.get_current_mode().await;
    let model_label = if current_mode == "offline" {
        "本地模型".to_string()
    } else {
        "DeepSeek V4 Flash".to_string()
    };

    Ok(AppResponse::ok(ChatMode {
        mode: current_mode,
        gateway_health: "ok".to_string(),
        inference_state: "running".to_string(),
        current_model_label: model_label,
    }))
}
