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
    pub token_count: Option<i64>,
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
        token_count: m.token_count,
        created_at: m.created_at,
    }
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<Vec<ChatSessionDto>>, String> {
    crate::storage::diag_log_pub("IPC list_sessions called");
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
    crate::storage::diag_log_pub(&format!("IPC create_chat_session called title={title:?}"));
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

#[derive(serde::Serialize)]
pub struct SessionSummaryDto {
    pub session_id: String,
    pub session_title: String,
    pub summary: String,
    pub updated_at: i64,
}

#[tauri::command]
pub async fn list_session_summaries(
    state: State<'_, crate::AppState>,
    limit: Option<i64>,
) -> Result<AppResponse<Vec<SessionSummaryDto>>, String> {
    let storage = state.storage.read().await;
    let lim = limit.unwrap_or(100);
    match storage.list_session_summaries(lim).await {
        Ok(rows) => Ok(AppResponse::ok(
            rows.into_iter()
                .map(|(session_id, session_title, summary, updated_at)| SessionSummaryDto {
                    session_id,
                    session_title,
                    summary,
                    updated_at,
                })
                .collect(),
        )),
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
    token_count: Option<i64>,
    state: State<'_, crate::AppState>,
) -> Result<AppResponse<ChatMessageDto>, String> {
    let storage = state.storage.read().await;
    let label = model_label.unwrap_or_else(|| "DeepSeek V4 Flash".to_string());
    match storage.complete_turn(&turn_id, &content, &label, token_count).await {
        Ok(message) => Ok(AppResponse::ok(dto_message(&message))),
        Err(e) => Ok(AppResponse::err("TURN_COMPLETE_FAILED", &e)),
    }
}

/// 长期记忆文档的完整内容（供设置页查看/编辑，也供发送消息前注入上下文）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDocDto {
    pub path: String,
    pub content: String,
    pub char_count: usize,
    pub inject_limit: usize,
    pub compact_threshold: usize,
}

/// 读取长期记忆文档（不存在时按模板创建）。
#[tauri::command]
pub async fn get_memory_doc() -> Result<AppResponse<MemoryDocDto>, String> {
    let path = crate::memory_doc::default_memory_path();
    match crate::memory_doc::read_raw(&path) {
        Ok(content) => Ok(AppResponse::ok(MemoryDocDto {
            path: path.display().to_string(),
            char_count: crate::memory_doc::char_count(&content),
            content,
            inject_limit: crate::memory_doc::INJECT_CHAR_LIMIT,
            compact_threshold: crate::memory_doc::COMPACT_CHAR_THRESHOLD,
        })),
        Err(e) => Ok(AppResponse::err("MEMORY_DOC_READ_FAILED", &e)),
    }
}

/// 保存用户手改（或模型整理后）的记忆文档内容，覆盖前自动留 .bak。
#[tauri::command]
pub async fn save_memory_doc(content: String) -> Result<AppResponse<usize>, String> {
    let path = crate::memory_doc::default_memory_path();
    match crate::memory_doc::save_user_edit(&path, &content) {
        Ok(chars) => Ok(AppResponse::ok(chars)),
        Err(e) => Ok(AppResponse::err("MEMORY_DOC_WRITE_FAILED", &e)),
    }
}

/// 追加长期事实（去重；同一 turn 只处理一次，重试不会重复写入）。
#[tauri::command]
pub async fn append_memory_facts(
    state: State<'_, crate::AppState>,
    turn_id: String,
    facts: Vec<String>,
) -> Result<AppResponse<usize>, String> {
    if facts.is_empty() {
        return Ok(AppResponse::ok(0));
    }
    let storage = state.storage.read().await;
    match storage.get_app_setting("memory_last_turn_id").await {
        Ok(Some(last)) if last == turn_id => return Ok(AppResponse::ok(0)),
        Ok(_) => {}
        Err(e) => return Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
    let path = crate::memory_doc::default_memory_path();
    let added = match crate::memory_doc::append_facts(&path, &facts) {
        Ok(n) => n,
        Err(e) => return Ok(AppResponse::err("MEMORY_DOC_WRITE_FAILED", &e)),
    };
    // 记录游标：写失败不影响本轮记忆已经落盘的事实
    if let Err(e) = storage.set_app_setting("memory_last_turn_id", &turn_id).await {
        crate::storage::diag_log_pub(&format!("append_memory_facts: 游标写入失败 {e}"));
    }
    Ok(AppResponse::ok(added))
}

/// 在资源管理器里定位记忆文档（路径由程序生成，不接受外部输入）。
#[tauri::command]
pub async fn open_memory_doc() -> Result<AppResponse<bool>, String> {
    let path = crate::memory_doc::default_memory_path();
    if let Err(e) = crate::memory_doc::ensure_file(&path) {
        return Ok(AppResponse::err("MEMORY_DOC_READ_FAILED", &e));
    }
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(format!("/select,{}", path.display()));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        match cmd.spawn() {
            Ok(_) => Ok(AppResponse::ok(true)),
            Err(e) => Ok(AppResponse::err("OPEN_FAILED", &format!("打开资源管理器失败: {e}"))),
        }
    }
    #[cfg(not(windows))]
    {
        Ok(AppResponse::err("UNSUPPORTED", "当前平台不支持打开资源管理器"))
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
