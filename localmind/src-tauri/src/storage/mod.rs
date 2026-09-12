// 存储管理模块 — SQLite 是 Session / Turn / Message / ToolCall 的持久化事实来源。
// Phase 0 只实现会话、Turn、消息和设置；其余表按 migration 预留。

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::chat::{ChatMessage, ChatSession};
use crate::common::AppResponse;

const CURRENT_SCHEMA_VERSION: i64 = 1;
const MIGRATION_001: &str = include_str!("../../migrations/001_init.sql");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub id: String,
    pub session_id: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStart {
    pub turn: TurnRecord,
    pub user_message: ChatMessage,
}

#[derive(Debug, Clone)]
pub struct StorageManager {
    db_path: PathBuf,
}

impl StorageManager {
    pub fn new() -> Result<Self, String> {
        Self::with_path(default_db_path()?)
    }

    pub fn with_path(db_path: PathBuf) -> Result<Self, String> {
        let had_existing_data = std::fs::metadata(&db_path)
            .map(|m| m.len() > 0)
            .unwrap_or(false);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建数据库目录失败: {}", e))?;
        }
        let mut conn = open_connection(&db_path)?;
        migrate(&mut conn, &db_path, had_existing_data)?;
        Ok(Self { db_path })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    async fn with_conn<T, F>(&self, f: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, String> + Send + 'static,
    {
        let path = self.db_path.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = open_connection(&path)?;
            f(&mut conn)
        })
        .await
        .map_err(|e| format!("数据库任务失败: {}", e))?
    }

    // ===== 会话操作 =====

    pub async fn get_sessions(&self) -> Result<Vec<ChatSession>, String> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT s.id, s.title, s.mode,
                            (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) AS message_count,
                            s.created_at, s.updated_at
                     FROM sessions s
                     ORDER BY s.updated_at DESC",
                )
                .map_err(db_error)?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(ChatSession {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        mode: row.get(2)?,
                        message_count: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                })
                .map_err(db_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
        })
        .await
    }

    pub async fn create_session(&self, title: String) -> Result<ChatSession, String> {
        self.with_conn(move |conn| {
            let now = chrono::Utc::now().timestamp_millis();
            let session = ChatSession {
                id: uuid::Uuid::new_v4().to_string(),
                title: if title.trim().is_empty() { "新对话".to_string() } else { title.trim().to_string() },
                mode: "online".to_string(),
                message_count: 0,
                created_at: now,
                updated_at: now,
            };
            conn.execute(
                "INSERT INTO sessions (id, title, mode, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![session.id, session.title, session.mode, session.created_at, session.updated_at],
            )
            .map_err(db_error)?;
            Ok(session)
        })
        .await
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<(), String> {
        let session_id = session_id.to_string();
        self.with_conn(move |conn| {
            let changed = conn
                .execute("DELETE FROM sessions WHERE id = ?1", params![session_id])
                .map_err(db_error)?;
            if changed == 0 {
                return Err("会话不存在".to_string());
            }
            Ok(())
        })
        .await
    }

    pub async fn rename_session(&self, session_id: &str, title: &str) -> Result<(), String> {
        let session_id = session_id.to_string();
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err("会话标题不能为空".to_string());
        }
        self.with_conn(move |conn| {
            let now = chrono::Utc::now().timestamp_millis();
            let changed = conn
                .execute(
                    "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
                    params![title, now, session_id],
                )
                .map_err(db_error)?;
            if changed == 0 {
                return Err("会话不存在".to_string());
            }
            Ok(())
        })
        .await
    }

    // ===== 消息操作 =====

    pub async fn get_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>, String> {
        let session_id = session_id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, session_id, role, content, model_label, created_at
                     FROM messages
                     WHERE session_id = ?1
                     ORDER BY created_at ASC, rowid ASC",
                )
                .map_err(db_error)?;
            let rows = stmt
                .query_map(params![session_id], |row| {
                    Ok(ChatMessage {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        model_label: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                })
                .map_err(db_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
        })
        .await
    }

    // ===== Turn / Message 原子操作 =====

    pub async fn begin_turn(
        &self,
        session_id: &str,
        user_content: &str,
        model_label: &str,
    ) -> Result<TurnStart, String> {
        let session_id = session_id.to_string();
        let user_content = user_content.to_string();
        let model_label = model_label.to_string();
        self.with_conn(move |conn| {
            let tx = conn.transaction().map_err(db_error)?;
            let session_exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM sessions WHERE id = ?1",
                    params![session_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_error)?;
            if session_exists.is_none() {
                return Err("会话不存在".to_string());
            }

            let now = chrono::Utc::now().timestamp_millis();
            let turn_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO turns (id, session_id, status, created_at, started_at, updated_at)
                 VALUES (?1, ?2, 'running', ?3, ?3, ?3)",
                params![turn_id, session_id, now],
            )
            .map_err(|e| {
                if e.to_string().contains("UNIQUE constraint failed") {
                    "同一会话已有运行中的 Turn，请等待完成或先取消".to_string()
                } else {
                    format!("创建 Turn 失败: {}", e)
                }
            })?;

            let message_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO messages (id, session_id, turn_id, role, content, model_label, created_at)
                 VALUES (?1, ?2, ?3, 'user', ?4, ?5, ?6)",
                params![message_id, session_id, turn_id, user_content, model_label, now],
            )
            .map_err(db_error)?;
            tx.execute(
                "UPDATE turns SET user_message_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![message_id, now, turn_id],
            )
            .map_err(db_error)?;
            tx.execute(
                "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
                params![now, session_id],
            )
            .map_err(db_error)?;
            tx.commit().map_err(db_error)?;

            Ok(TurnStart {
                turn: TurnRecord {
                    id: turn_id,
                    session_id: session_id.clone(),
                    status: "running".to_string(),
                    created_at: now,
                    updated_at: now,
                },
                user_message: ChatMessage {
                    id: message_id,
                    session_id,
                    role: "user".to_string(),
                    content: user_content,
                    model_label,
                    created_at: now,
                },
            })
        })
        .await
    }

    pub async fn complete_turn(
        &self,
        turn_id: &str,
        assistant_content: &str,
        model_label: &str,
    ) -> Result<ChatMessage, String> {
        let turn_id = turn_id.to_string();
        let assistant_content = assistant_content.to_string();
        let model_label = model_label.to_string();
        self.with_conn(move |conn| {
            let tx = conn.transaction().map_err(db_error)?;
            let record: Option<(String, String)> = tx
                .query_row(
                    "SELECT session_id, status FROM turns WHERE id = ?1",
                    params![turn_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(db_error)?;
            let Some((session_id, status)) = record else {
                return Err("Turn 不存在".to_string());
            };
            if status != "running" {
                return Err(format!("Turn 状态不允许完成: {}", status));
            }

            let now = chrono::Utc::now().timestamp_millis();
            let message_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO messages (id, session_id, turn_id, role, content, model_label, created_at)
                 VALUES (?1, ?2, ?3, 'assistant', ?4, ?5, ?6)",
                params![message_id, session_id, turn_id, assistant_content, model_label, now],
            )
            .map_err(db_error)?;
            tx.execute(
                "UPDATE turns SET status = 'completed', assistant_message_id = ?1,
                 completed_at = ?2, updated_at = ?2 WHERE id = ?3",
                params![message_id, now, turn_id],
            )
            .map_err(db_error)?;
            tx.execute(
                "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
                params![now, session_id],
            )
            .map_err(db_error)?;
            tx.commit().map_err(db_error)?;

            Ok(ChatMessage {
                id: message_id,
                session_id,
                role: "assistant".to_string(),
                content: assistant_content,
                model_label,
                created_at: now,
            })
        })
        .await
    }

    pub async fn fail_turn(
        &self,
        turn_id: &str,
        status: &str,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), String> {
        if !matches!(status, "failed" | "cancelled" | "interrupted") {
            return Err("Turn 失败状态只能是 failed/cancelled/interrupted".to_string());
        }
        let turn_id = turn_id.to_string();
        let status = status.to_string();
        let error_code = error_code.to_string();
        let error_message = error_message.to_string();
        self.with_conn(move |conn| {
            let now = chrono::Utc::now().timestamp_millis();
            let changed = conn
                .execute(
                    "UPDATE turns SET status = ?1, error_code = ?2, error_message = ?3,
                     completed_at = ?4, updated_at = ?4
                     WHERE id = ?5 AND status = 'running'",
                    params![status, error_code, error_message, now, turn_id],
                )
                .map_err(db_error)?;
            if changed == 0 {
                return Err("Turn 不存在或已经结束".to_string());
            }
            Ok(())
        })
        .await
    }

    // ===== 设置 / 设备身份 =====


    pub async fn get_api_key(&self) -> Result<Option<String>, String> {
        self.with_conn(|conn| {
            let stored = get_setting(conn, "deepseek_api_key")?;
            match stored {
                Some(val) => {
                    let device_id = get_setting(conn, "device_id")?.unwrap_or_default();
                    if val.starts_with("ENC:") {
                        let decrypted = crate::crypto_util::decrypt_string(&val[4..], &device_id)?;
                        Ok(Some(decrypted))
                    } else {
                        Ok(Some(val))
                    }
                }
                None => Ok(None),
            }
        })
        .await
    }

    pub async fn set_api_key(&self, key: String) -> Result<(), String> {
        self.with_conn(move |conn| upsert_setting(conn, "deepseek_api_key", &key))
            .await
    }
    pub async fn get_memories(&self, limit: i64) -> Result<Vec<(String, String, String, f64)>, String> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, category, content, confidence FROM memory_entries ORDER BY updated_at DESC LIMIT ?1"
            ).map_err(|e| format!("database error: {e}"))?;
            let rows = stmt.query_map(rusqlite::params![limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                ))
            }).map_err(|e| format!("database error: {e}"))?;
            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| format!("database error: {e}"))?);
            }
            Ok(results)
        })
        .await
    }

    pub async fn save_memory(&self, category: &str, content: &str, source_session_id: Option<&str>) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let cat = category.to_string();
        let con = content.to_string();
        let sid = source_session_id.map(|s| s.to_string());
        let now = chrono::Utc::now().timestamp_millis();
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO memory_entries (id, category, content, confidence, source_session_id, created_at, updated_at, access_count) VALUES (?1, ?2, ?3, 0.8, ?4, ?5, ?5, 0)",
                rusqlite::params![id, cat, con, sid, now],
            ).map_err(|e| format!("database error: {e}"))?;
            Ok(id)
        })
        .await
    }

    pub async fn delete_memory(&self, memory_id: &str) -> Result<(), String> {
        let mid = memory_id.to_string();
        self.with_conn(move |conn| {
            conn.execute("DELETE FROM memory_entries WHERE id = ?1", rusqlite::params![mid])
                .map_err(|e| format!("database error: {e}"))?;
            Ok(())
        })
        .await
    }

    pub async fn search_memories(&self, keyword: &str, limit: i64) -> Result<Vec<(String, String, String, f64)>, String> {
        let kw = format!("%{}%", keyword);
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, category, content, confidence FROM memory_entries WHERE content LIKE ?1 ORDER BY confidence DESC, updated_at DESC LIMIT ?2"
            ).map_err(|e| format!("database error: {e}"))?;
            let rows = stmt.query_map(rusqlite::params![kw, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                ))
            }).map_err(|e| format!("database error: {e}"))?;
            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| format!("database error: {e}"))?);
            }
            Ok(results)
        })
        .await
    }
    pub async fn save_audit_log(
        &self,
        session_id: Option<&str>,
        action: &str,
        target: Option<&str>,
        risk_level: &str,
        result: &str,
        detail: Option<&str>,
    ) -> Result<(), String> {
        let sid = session_id.map(|s| s.to_string());
        let act = action.to_string();
        let tgt = target.map(|s| s.to_string());
        let rl = risk_level.to_string();
        let res = result.to_string();
        let det = detail.map(|s| s.to_string());
        let now = chrono::Utc::now().timestamp_millis();
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO audit_log (id, session_id, action, target, risk_level, result, detail, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), sid, act, tgt, rl, res, det, now],
            ).map_err(|e| format!("database error: {e}"))?;
            Ok(())
        })
        .await
    }
    pub async fn get_session_summary(&self, session_id: &str) -> Result<Option<String>, String> {
        let sid = session_id.to_string();
        self.with_conn(move |conn| {
            conn.query_row(
                "SELECT summary FROM session_summaries WHERE session_id = ?1",
                rusqlite::params![sid],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| format!("database error: {e}"))
        })
        .await
    }

    pub async fn save_session_summary(&self, session_id: &str, summary: &str) -> Result<(), String> {
        let sid = session_id.to_string();
        let sum = summary.to_string();
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO session_summaries (session_id, summary, covers_until_message_id, token_count, created_at, updated_at) VALUES (?1, ?2, '', 0, ?3, ?3) ON CONFLICT(session_id) DO UPDATE SET summary = excluded.summary, updated_at = excluded.updated_at",
                rusqlite::params![sid, sum, chrono::Utc::now().timestamp_millis()],
            )
            .map_err(|e| format!("database error: {e}"))?;
            Ok(())
        })
        .await
    }
    pub async fn get_device_id(&self) -> Result<String, String> {
        self.with_conn(|conn| {
            if let Some(value) = get_setting(conn, "device_id")? {
                return Ok(value);
            }
            let value = uuid::Uuid::new_v4().to_string();
            upsert_setting(conn, "device_id", &value)?;
            Ok(value)
        })
        .await
    }
}

fn default_db_path() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("AppData/Roaming")))
        .ok_or_else(|| "无法确定 APPDATA 路径".to_string())?;
    Ok(appdata.join("LocalMind").join("localmind.db"))
}

fn open_connection(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(db_error)?;
    conn.busy_timeout(Duration::from_millis(5000)).map_err(db_error)?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(db_error)?;
    conn.pragma_update(None, "synchronous", "NORMAL").map_err(db_error)?;
    let _: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(db_error)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection, db_path: &Path, had_existing_data: bool) -> Result<(), String> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(db_error)?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(format!("数据库版本 {} 高于当前支持版本 {}", version, CURRENT_SCHEMA_VERSION));
    }
    if version < 1 {
        if had_existing_data {
            backup_before_migration(conn, db_path, version)?;
        }
        conn.execute_batch(MIGRATION_001).map_err(db_error)?;
        conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .map_err(db_error)?;
    }
    Ok(())
}

fn backup_before_migration(conn: &Connection, db_path: &Path, version: i64) -> Result<(), String> {
    let file_name = db_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("localmind.db");
    let mut backup_path = db_path.with_file_name(format!("{}.bak.v{}", file_name, version));
    if backup_path.exists() {
        let suffix = chrono::Utc::now().format("%Y%m%d%H%M%S");
        backup_path = db_path.with_file_name(format!("{}.bak.v{}.{}", file_name, version, suffix));
    }
    let mut destination = Connection::open(&backup_path).map_err(db_error)?;
    {
        let backup = rusqlite::backup::Backup::new(conn, &mut destination).map_err(db_error)?;
        backup
            .run_to_completion(5, Duration::from_millis(50), None)
            .map_err(db_error)?;
    }
    Ok(())
}

fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(db_error)
}

fn upsert_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, chrono::Utc::now().timestamp_millis()],
    )
    .map_err(db_error)?;
    Ok(())
}

fn db_error(error: rusqlite::Error) -> String {
    format!("数据库错误: {}", error)
}

// ========== IPC Commands ==========

#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub device_id: String,
}

#[tauri::command]
pub async fn get_config() -> Result<AppResponse<crate::common::AppConfig>, String> {
    Ok(AppResponse::ok(crate::common::AppConfig::load()))
}

#[tauri::command]
pub async fn save_config() -> Result<AppResponse<bool>, String> {
    Ok(AppResponse::ok(true))
}

#[tauri::command]
pub async fn get_app_info(
    state: tauri::State<'_, crate::AppState>,
) -> Result<AppResponse<AppInfo>, String> {
    let storage = state.storage.read().await;
    match storage.get_device_id().await {
        Ok(device_id) => Ok(AppResponse::ok(AppInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            device_id,
        })),
        Err(e) => Ok(AppResponse::err("STORAGE_ERROR", &e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> (tempfile::TempDir, StorageManager) {
        let dir = tempfile::tempdir().unwrap();
        let manager = StorageManager::with_path(dir.path().join("localmind.db")).unwrap();
        (dir, manager)
    }

    #[tokio::test]
    async fn session_and_messages_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("localmind.db");
        let manager = StorageManager::with_path(path.clone()).unwrap();
        let session = manager.create_session("测试".to_string()).await.unwrap();
        let start = manager.begin_turn(&session.id, "你好", "test-model").await.unwrap();
        manager
            .complete_turn(&start.turn.id, "你好，我在这里", "test-model")
            .await
            .unwrap();
        drop(manager);

        let reopened = StorageManager::with_path(path).unwrap();
        let messages = reopened.get_messages(&session.id).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "你好，我在这里");
    }

    #[tokio::test]
    async fn one_running_turn_per_session() {
        let (_dir, manager) = manager();
        let session = manager.create_session("并发".to_string()).await.unwrap();
        manager.begin_turn(&session.id, "第一次", "m").await.unwrap();
        let second = manager.begin_turn(&session.id, "第二次", "m").await;
        assert!(second.is_err());
    }

    #[tokio::test]
    async fn device_id_is_stable() {
        let (_dir, manager) = manager();
        let first = manager.get_device_id().await.unwrap();
        let second = manager.get_device_id().await.unwrap();
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn delete_session_cascades() {
        let (_dir, manager) = manager();
        let session = manager.create_session("删除".to_string()).await.unwrap();
        let start = manager.begin_turn(&session.id, "内容", "m").await.unwrap();
        manager.complete_turn(&start.turn.id, "回复", "m").await.unwrap();
        manager.delete_session(&session.id).await.unwrap();

        let conn = open_connection(manager.db_path()).unwrap();
        let sessions: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        let messages: i64 = conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0)).unwrap();
        let turns: i64 = conn.query_row("SELECT COUNT(*) FROM turns", [], |r| r.get(0)).unwrap();
        assert_eq!((sessions, messages, turns), (0, 0, 0));
    }
}
