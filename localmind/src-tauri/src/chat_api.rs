use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tauri::State;
use crate::common::AppResponse;
use crate::storage::StorageManager;
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct ChatSessionDto { pub id: String, pub title: String, pub created_at: i64, pub updated_at: i64, pub message_count: i32, pub mode: String }
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct ChatMessageDto { pub id: String, pub session_id: String, pub role: String, pub content: String, pub model_label: String, pub created_at: i64 }
fn dto_session(s: &crate::chat::ChatSession) -> ChatSessionDto { ChatSessionDto { id: s.id.clone(), title: s.title.clone(), created_at: s.created_at, updated_at: s.updated_at, message_count: s.message_count, mode: s.mode.clone() } }
fn dto_message(m: &crate::chat::ChatMessage) -> ChatMessageDto { ChatMessageDto { id: m.id.clone(), session_id: m.session_id.clone(), role: m.role.clone(), content: m.content.clone(), model_label: m.model_label.clone(), created_at: m.created_at } }
#[tauri::command] pub async fn list_sessions(state: State<'_, crate::AppState>) -> Result<AppResponse<Vec<ChatSessionDto>>, String> { let storage = state.storage.read().await; let sessions = storage.get_sessions().await; Ok(AppResponse::ok(sessions.iter().map(dto_session).collect())) }
#[tauri::command] pub async fn create_chat_session(title: Option<String>, state: State<'_, crate::AppState>) -> Result<AppResponse<ChatSessionDto>, String> { let storage = state.storage.read().await; let session = storage.create_session(title.unwrap_or_default()).await; Ok(AppResponse::ok(dto_session(&session))) }
#[tauri::command] pub async fn delete_chat_session(session_id: String, state: State<'_, crate::AppState>) -> Result<AppResponse<bool>, String> { let storage = state.storage.read().await; storage.delete_session(&session_id).await; Ok(AppResponse::ok(true)) }
#[tauri::command] pub async fn rename_chat_session(session_id: String, title: String, state: State<'_, crate::AppState>) -> Result<AppResponse<bool>, String> { let storage = state.storage.read().await; storage.rename_session(&session_id, &title).await; Ok(AppResponse::ok(true)) }
#[tauri::command] pub async fn list_messages(session_id: String, state: State<'_, crate::AppState>) -> Result<AppResponse<Vec<ChatMessageDto>>, String> { let storage = state.storage.read().await; let msgs = storage.get_messages(&session_id).await; Ok(AppResponse::ok(msgs.iter().map(dto_message).collect())) }
#[tauri::command] pub async fn append_message(session_id: String, role: String, content: String, model_label: String, state: State<'_, crate::AppState>) -> Result<AppResponse<ChatMessageDto>, String> { let storage = state.storage.read().await; let msg = storage.save_message(&session_id, &role, &content, &model_label).await; Ok(AppResponse::ok(dto_message(&msg))) }
