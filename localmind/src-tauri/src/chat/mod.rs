// 对话引擎模块 — M1
// 在线/离线双模对话、会话管理、流式输出
// 对齐：系统设计 §3.2.M1

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tauri::State;
use reqwest::Client;

use crate::common::{AppConfig, AppResponse};
use crate::storage::StorageManager;

/// 会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub mode: String,
    pub message_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub model_label: String,
    pub created_at: i64,
}

/// 模式状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMode {
    pub mode: String,
    pub gateway_health: String,
    pub inference_state: String,
    pub current_model_label: String,
}

/// 对话引擎
pub struct ChatEngine {
    config: AppConfig,
    storage: Arc<RwLock<StorageManager>>,
    current_mode: RwLock<String>,
    http_client: Client,
    generation_id: RwLock<String>,
    is_streaming: RwLock<bool>,
}

impl ChatEngine {
    pub fn new(storage: Arc<RwLock<StorageManager>>) -> Self {
        Self {
            config: AppConfig::load(),
            storage,
            current_mode: RwLock::new("online".to_string()),
            http_client: Client::new(),
            generation_id: RwLock::new(String::new()),
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
        let mut current = self.current_mode.write().await;
        *current = mode.to_string();
        Ok(mode.to_string())
    }
}

// ========== IPC Commands ==========

#[tauri::command]
pub async fn get_sessions(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<Vec<ChatSession>>, String> {
    let storage = state.storage.read().await;
    let sessions = storage.get_sessions().await;
    Ok(AppResponse::ok(sessions))
}

#[tauri::command]
pub async fn create_session(
    title: Option<String>,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<ChatSession>, String> {
    let storage = state.storage.read().await;
    let session = storage.create_session(title.unwrap_or_default()).await;
    Ok(AppResponse::ok(session))
}

#[tauri::command]
pub async fn delete_session(
    session_id: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    storage.delete_session(&session_id).await;
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn rename_session(
    session_id: String,
    title: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    storage.rename_session(&session_id, &title).await;
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn send_message(
    session_id: String,
    content: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<ChatMessage>, String> {
    let engine = state.chat_engine.read().await;
    let mode = engine.get_current_mode().await;

    let storage = state.storage.read().await;
    let model_label = match mode.as_str() {
        "offline" => "本地模型".to_string(),
        _ => "Deepseek-V4-Pro".to_string(),
    };

    // 保存用户消息
    let user_msg = storage
        .save_message(&session_id, "user", &content, &model_label)
        .await;

    // AI 回复（模拟 — 实际应调推理/网关）
    let ai_content = format!("（{}模式）已收到您的消息：{}", mode, content);
    let ai_msg = storage
        .save_message(&session_id, "assistant", &ai_content, &model_label)
        .await;

    Ok(AppResponse::ok(ai_msg))
}

#[tauri::command]
pub async fn stop_generation(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<bool>, String> {
    let engine = state.chat_engine.read().await;
    let mut streaming = engine.is_streaming.write().await;
    *streaming = false;
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn get_messages(
    session_id: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<Vec<ChatMessage>>, String> {
    let storage = state.storage.read().await;
    let messages = storage.get_messages(&session_id).await;
    Ok(AppResponse::ok(messages))
}

#[tauri::command]
pub async fn switch_mode(
    mode: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<ChatMode>, String> {
    let engine = state.chat_engine.read().await;
    let current_mode = engine.set_mode(&mode).await.map_err(|e| e)?;

    let model_label = match current_mode.as_str() {
        "offline" => "本地模型".to_string(),
        _ => "Deepseek-V4-Pro".to_string(),
    };

    // 检查推理状态
    let inference_state = if current_mode == "offline" {
        let model_mgr = state.model_manager.read().await;
        model_mgr.get_state().await
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
    let model_label = match current_mode.as_str() {
        "offline" => "本地模型".to_string(),
        _ => "Deepseek-V4-Pro".to_string(),
    };

    Ok(AppResponse::ok(ChatMode {
        mode: current_mode,
        gateway_health: "ok".to_string(),
        inference_state: "running".to_string(),
        current_model_label: model_label,
    }))
}
