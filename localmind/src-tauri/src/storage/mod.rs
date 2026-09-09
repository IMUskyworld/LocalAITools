// 存储管理模块 — C-02 端侧本地存储
// 会话/消息/模型资产/白名单配置/指令日志的 SQLite 访问
// 对齐：系统设计 §4.2 端侧表结构

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::chat::{ChatMessage, ChatSession};
use crate::common::AppResponse;

/// 存储管理器（MVP 使用内存存储，正式版切换至 SQLite）
pub struct StorageManager {
    sessions: RwLock<Vec<ChatSession>>,
    messages: RwLock<Vec<ChatMessage>>,
}

impl StorageManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(Vec::new()),
            messages: RwLock::new(Vec::new()),
        }
    }

    // ===== 会话操作 =====

    pub async fn get_sessions(&self) -> Vec<ChatSession> {
        let sessions = self.sessions.read().await;
        let mut result = sessions.clone();
        result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        result
    }

    pub async fn create_session(&self, title: String) -> ChatSession {
        let now = chrono::Utc::now().timestamp_millis();
        let session = ChatSession {
            id: uuid::Uuid::new_v4().to_string(),
            title: if title.is_empty() { "新对话".to_string() } else { title },
            mode: "online".to_string(),
            message_count: 0,
            created_at: now,
            updated_at: now,
        };
        let mut sessions = self.sessions.write().await;
        sessions.push(session.clone());
        session
    }

    pub async fn delete_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.retain(|s| s.id != session_id);
        let mut messages = self.messages.write().await;
        messages.retain(|m| m.session_id != session_id);
    }

    pub async fn rename_session(&self, session_id: &str, title: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
            session.title = title.to_string();
            session.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }

    // ===== 消息操作 =====

    pub async fn get_messages(&self, session_id: &str) -> Vec<ChatMessage> {
        let messages = self.messages.read().await;
        let mut result: Vec<ChatMessage> = messages
            .iter()
            .filter(|m| m.session_id == session_id)
            .cloned()
            .collect();
        result.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        result
    }

    pub async fn save_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        model_label: &str,
    ) -> ChatMessage {
        let now = chrono::Utc::now().timestamp_millis();
        let msg = ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            model_label: model_label.to_string(),
            created_at: now,
        };
        let mut messages = self.messages.write().await;
        messages.push(msg.clone());

        // 更新会话消息数
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
            session.message_count += 1;
            session.updated_at = now;
        }

        msg
    }
}

// ========== IPC Commands ==========

#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub device_id: String,
}

#[tauri::command]
pub async fn get_config(
) -> Result<AppResponse<crate::common::AppConfig>, String> {
    let config = crate::common::AppConfig::load();
    Ok(AppResponse::ok(config))
}

#[tauri::command]
pub async fn save_config(
) -> Result<AppResponse<bool>, String> {
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn get_app_info(
) -> Result<AppResponse<AppInfo>, String> {
    Ok(AppResponse::ok(AppInfo {
        version: "0.1.0".to_string(),
        device_id: uuid::Uuid::new_v4().to_string(),
    }))
}
