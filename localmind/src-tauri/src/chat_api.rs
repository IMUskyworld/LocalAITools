use serde::{Deserialize, Serialize};
use tauri::State;

use crate::chat::{ChatMessage, ChatSession};
use crate::common::AppResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSessionDto {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: i32,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageDto {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub model_label: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStartDto {
    pub turn_id: String,
    pub session_id: String,
    pub status: String,
    pub created_at: i64,
    pub user_message: ChatMessageDto,
}

fn dto_session(s: &ChatSession) -> ChatSessionDto {
    ChatSessionDto {
        id: s.id.clone(),
        title: s.title.clone(),
        created_at: s.created_at,
        updated_at: s.updated_at,
        message_count: s.message_count,
        mode: s.mode.clone(),
    }
}

fn dto_message(m: &ChatMessage) -> ChatMessageDto {
    ChatMessageDto {
        id: m.id.clone(),
        session_id: m.session_id.clone(),
        role: m.role.clone(),
        content: m.content.clone(),
        model_label: m.model_label.clone(),
        created_at: m.created_at,
    }
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<Vec<ChatSessionDto>>, String> {
    let storage = state.storage.read().await;
    match storage.get_sessions().await {
        Ok(sessions) => Ok(AppResponse::ok(sessions.iter().map(dto_session).collect())),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn create_chat_session(
    title: Option<String>,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<ChatSessionDto>, String> {
    let storage = state.storage.read().await;
    match storage.create_session(title.unwrap_or_default()).await {
        Ok(session) => Ok(AppResponse::ok(dto_session(&session))),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn delete_chat_session(
    session_id: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    match storage.delete_session(&session_id).await {
        Ok(()) => Ok(AppResponse::ok(true)),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn rename_chat_session(
    session_id: String,
    title: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    match storage.rename_session(&session_id, &title).await {
        Ok(()) => Ok(AppResponse::ok(true)),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn list_messages(
    session_id: String,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<Vec<ChatMessageDto>>, String> {
    let storage = state.storage.read().await;
    match storage.get_messages(&session_id).await {
        Ok(messages) => Ok(AppResponse::ok(messages.iter().map(dto_message).collect())),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn get_api_key(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<String>, String> {
    let storage = state.storage.read().await;
    match storage.get_api_key().await {
        Ok(Some(key)) => Ok(AppResponse::ok(key)),
        Ok(None) => Ok(AppResponse::ok(String::new())),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn set_api_key(
    state: State<'_, crate::AppState>,
    key: String,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    match storage.set_api_key(key.clone()).await {
        Ok(()) => Ok(AppResponse::ok(true)),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn save_audit_log(
    state: State<'_, crate::AppState>,
    session_id: Option<String>,
    action: String,
    target: Option<String>,
    risk_level: String,
    result: String,
    detail: Option<String>,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    match storage.save_audit_log(
        session_id.as_deref(),
        &action,
        target.as_deref(),
        &risk_level,
        &result,
        detail.as_deref(),
    ).await {
        Ok(()) => Ok(AppResponse::ok(true)),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}
#[tauri::command]
pub async fn get_session_summary(
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<AppResponse<String>, String> {
    let storage = state.storage.read().await;
    match storage.get_session_summary(&session_id).await {
        Ok(Some(s)) => Ok(AppResponse::ok(s)),
        Ok(None) => Ok(AppResponse::ok(String::new())),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn save_session_summary(
    state: State<'_, crate::AppState>,
    session_id: String,
    summary: String,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    match storage.save_session_summary(&session_id, &summary).await {
        Ok(()) => Ok(AppResponse::ok(true)),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}
#[derive(serde::Serialize)]
pub struct MemoryEntryDto {
    pub id: String,
    pub category: String,
    pub content: String,
    pub confidence: f64,
}

#[tauri::command]
pub async fn get_memories(
    state: State<'_, crate::AppState>,
    limit: Option<i64>,
) -> Result<AppResponse<Vec<MemoryEntryDto>>, String> {
    let storage = state.storage.read().await;
    let lim = limit.unwrap_or(50);
    match storage.get_memories(lim).await {
        Ok(entries) => Ok(AppResponse::ok(entries.into_iter().map(|(id, category, content, confidence)| MemoryEntryDto { id, category, content, confidence }).collect())),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn save_memory(
    state: State<'_, crate::AppState>,
    category: String,
    content: String,
    session_id: Option<String>,
) -> Result<AppResponse<String>, String> {
    let storage = state.storage.read().await;
    match storage.save_memory(&category, &content, session_id.as_deref()).await {
        Ok(id) => Ok(AppResponse::ok(id)),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn delete_memory(
    state: State<'_, crate::AppState>,
    memory_id: String,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    match storage.delete_memory(&memory_id).await {
        Ok(()) => Ok(AppResponse::ok(true)),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[tauri::command]
pub async fn search_memories(
    state: State<'_, crate::AppState>,
    keyword: String,
    limit: Option<i64>,
) -> Result<AppResponse<Vec<MemoryEntryDto>>, String> {
    let storage = state.storage.read().await;
    let lim = limit.unwrap_or(20);
    match storage.search_memories(&keyword, lim).await {
        Ok(entries) => Ok(AppResponse::ok(entries.into_iter().map(|(id, category, content, confidence)| MemoryEntryDto { id, category, content, confidence }).collect())),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}
#[tauri::command]
pub async fn turn_begin(
    session_id: String,
    content: String,
    model_label: Option<String>,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<TurnStartDto>, String> {
    let storage = state.storage.read().await;
    let label = model_label.unwrap_or_else(|| "DeepSeek V4 Flash".to_string());
    match storage.begin_turn(&session_id, &content, &label).await {
        Ok(start) => Ok(AppResponse::ok(TurnStartDto {
            turn_id: start.turn.id,
            session_id: start.turn.session_id,
            status: start.turn.status,
            created_at: start.turn.created_at,
            user_message: dto_message(&start.user_message),
        })),
        Err(e) => Ok(AppResponse::err("TURN_CONFLICT_OR_STORAGE", &e)),
    }
}

#[tauri::command]
pub async fn turn_complete(
    turn_id: String,
    content: String,
    model_label: Option<String>,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<ChatMessageDto>, String> {
    let storage = state.storage.read().await;
    let label = model_label.unwrap_or_else(|| "DeepSeek V4 Flash".to_string());
    match storage.complete_turn(&turn_id, &content, &label).await {
        Ok(message) => Ok(AppResponse::ok(dto_message(&message))),
        Err(e) => Ok(AppResponse::err("TURN_COMPLETE_FAILED", &e)),
    }
}

#[tauri::command]
pub async fn turn_fail(
    turn_id: String,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<bool>, String> {
    let storage = state.storage.read().await;
    match storage
        .fail_turn(
            &turn_id,
            &status,
            error_code.as_deref().unwrap_or("AGENT_ERROR"),
            error_message.as_deref().unwrap_or(""),
        )
        .await
    {
        Ok(()) => Ok(AppResponse::ok(true)),
        Err(e) => Ok(AppResponse::err("TURN_FAILED_UPDATE_ERROR", &e)),
    }
}
