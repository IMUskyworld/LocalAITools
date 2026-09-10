use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use localmind_shared_contract::{CommandEnvelope, CommandState};
use parking_lot::Mutex;
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::error::{RelayError, Result};
use crate::protocol::{ERROR_INVALID_PAIRING_CODE, ERROR_PAIRING_CODE_USED, ERROR_UNAUTHORIZED};

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS devices (
    id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL CHECK (platform IN ('windows', 'android')),
    model TEXT NOT NULL DEFAULT '',
    os_version TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS pairing_codes (
    code TEXT PRIMARY KEY,
    issued_by TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    claimed_by TEXT REFERENCES devices(id) ON DELETE SET NULL,
    claimed_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_pairing_codes_issued_by
    ON pairing_codes(issued_by, expires_at);

CREATE TABLE IF NOT EXISTS pairings (
    tenant_id TEXT PRIMARY KEY,
    windows_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    android_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    revoked_at INTEGER,
    UNIQUE (windows_device_id, android_device_id)
);

CREATE INDEX IF NOT EXISTS idx_pairings_windows
    ON pairings(windows_device_id, revoked_at);
CREATE INDEX IF NOT EXISTS idx_pairings_android
    ON pairings(android_device_id, revoked_at);

CREATE TABLE IF NOT EXISTS commands (
    command_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES pairings(tenant_id) ON DELETE CASCADE,
    from_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    to_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL,
    envelope_json TEXT NOT NULL,
    state TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_commands_target_state
    ON commands(to_device_id, state, updated_at);

CREATE TABLE IF NOT EXISTS relay_messages (
    id TEXT PRIMARY KEY,
    target_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    envelope_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    delivered_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_relay_messages_pending
    ON relay_messages(target_device_id, delivered_at, created_at);
"#;

#[derive(Clone, Debug, Serialize)]
pub struct Device {
    pub id: String,
    pub device_name: String,
    pub platform: String,
    pub model: String,
    pub os_version: String,
    pub created_at: i64,
    pub last_seen: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Pairing {
    pub tenant_id: String,
    pub peer: Device,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub struct Route;

#[derive(Clone, Debug)]
pub struct PendingMessage {
    pub id: String,
    pub envelope_json: String,
}

#[derive(Clone, Debug)]
pub struct CommandRecord {
    pub inserted: bool,
    pub state: String,
    pub envelope_json: String,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    path: Option<PathBuf>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| {
                    RelayError::Internal(format!("failed to create database directory: {err}"))
                })?;
            }
        }

        let conn = Connection::open(&path)?;
        configure_connection(&conn)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: Some(path),
        })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        configure_connection(&conn)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: None,
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn register_device(
        &self,
        device_name: &str,
        platform: &str,
        model: &str,
        os_version: &str,
    ) -> Result<(Device, String)> {
        let id = Uuid::new_v4().to_string();
        let token = random_token();
        let token_hash = hash_token(&token);
        let now = now_ms();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO devices (id, token_hash, device_name, platform, model, os_version, created_at, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![id, token_hash, device_name, platform, model, os_version, now],
        )?;
        Ok((
            Device {
                id,
                device_name: device_name.to_string(),
                platform: platform.to_string(),
                model: model.to_string(),
                os_version: os_version.to_string(),
                created_at: now,
                last_seen: now,
            },
            token,
        ))
    }

    pub fn authenticate(&self, device_id: &str, token: &str) -> Result<Device> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "SELECT id, token_hash, device_name, platform, model, os_version, created_at, last_seen
                 FROM devices WHERE id = ?1",
                params![device_id],
                |row| {
                    Ok((
                        Device {
                            id: row.get(0)?,
                            device_name: row.get(2)?,
                            platform: row.get(3)?,
                            model: row.get(4)?,
                            os_version: row.get(5)?,
                            created_at: row.get(6)?,
                            last_seen: row.get(7)?,
                        },
                        row.get::<_, String>(1)?,
                    ))
                },
            )
            .optional()?;

        let Some((mut device, expected_hash)) = row else {
            return Err(RelayError::unauthorized(
                ERROR_UNAUTHORIZED,
                "unknown device",
            ));
        };

        let actual_hash = hash_token(token);
        if !constant_time_eq(actual_hash.as_bytes(), expected_hash.as_bytes()) {
            return Err(RelayError::unauthorized(
                ERROR_UNAUTHORIZED,
                "invalid device token",
            ));
        }

        let now = now_ms();
        conn.execute(
            "UPDATE devices SET last_seen = ?1 WHERE id = ?2",
            params![now, device.id],
        )?;
        device.last_seen = now;
        Ok(device)
    }

    pub fn touch_device(&self, device_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE devices SET last_seen = ?1 WHERE id = ?2",
            params![now_ms(), device_id],
        )?;
        Ok(())
    }

    pub fn issue_pairing_code(&self, issued_by: &str, ttl_seconds: i64) -> Result<(String, i64)> {
        let now = now_ms();
        let expires_at = now + ttl_seconds.max(60) * 1_000;
        let conn = self.conn.lock();

        conn.execute(
            "DELETE FROM pairing_codes WHERE expires_at < ?1 OR claimed_at IS NOT NULL",
            params![now],
        )?;

        for _ in 0..32 {
            let code = format!("{:06}", rand::thread_rng().next_u32() % 1_000_000);
            let result = conn.execute(
                "INSERT OR IGNORE INTO pairing_codes (code, issued_by, created_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![code, issued_by, now, expires_at],
            )?;
            if result == 1 {
                return Ok((code, expires_at));
            }
        }

        Err(RelayError::Internal(
            "failed to allocate a unique pairing code".to_string(),
        ))
    }

    pub fn claim_pairing_code(&self, claimed_by: &str, code: &str) -> Result<Pairing> {
        let now = now_ms();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let row = tx
            .query_row(
                "SELECT issued_by, expires_at, claimed_by
                 FROM pairing_codes WHERE code = ?1",
                params![code],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;

        let Some((issued_by, expires_at, claimed_by_existing)) = row else {
            return Err(RelayError::bad_request(
                ERROR_INVALID_PAIRING_CODE,
                "pairing code is invalid",
            ));
        };
        if expires_at < now {
            return Err(RelayError::bad_request(
                ERROR_INVALID_PAIRING_CODE,
                "pairing code has expired",
            ));
        }
        if claimed_by_existing.is_some() {
            return Err(RelayError::conflict(
                ERROR_PAIRING_CODE_USED,
                "pairing code has already been used",
            ));
        }
        if issued_by == claimed_by {
            return Err(RelayError::bad_request(
                ERROR_INVALID_PAIRING_CODE,
                "a device cannot pair with itself",
            ));
        }

        let issuer = get_device(&tx, &issued_by)?;
        let claimant = get_device(&tx, claimed_by)?;
        let (windows_device_id, android_device_id) =
            match (issuer.platform.as_str(), claimant.platform.as_str()) {
                ("windows", "android") => (issuer.id.clone(), claimant.id.clone()),
                ("android", "windows") => (claimant.id.clone(), issuer.id.clone()),
                _ => {
                    return Err(RelayError::forbidden(
                        ERROR_UNAUTHORIZED,
                        "pairing requires one Windows device and one Android device",
                    ));
                }
            };

        if let Some(existing) = find_pair_by_devices(&tx, &windows_device_id, &android_device_id)? {
            tx.execute(
                "UPDATE pairing_codes SET claimed_by = ?1, claimed_at = ?2 WHERE code = ?3",
                params![claimed_by, now, code],
            )?;
            tx.commit()?;
            return Ok(Pairing {
                tenant_id: existing.tenant_id,
                peer: issuer,
                created_at: existing.created_at,
            });
        }

        let tenant_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO pairings (tenant_id, windows_device_id, android_device_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![tenant_id, windows_device_id, android_device_id, now],
        )?;
        tx.execute(
            "UPDATE pairing_codes SET claimed_by = ?1, claimed_at = ?2 WHERE code = ?3",
            params![claimed_by, now, code],
        )?;
        tx.commit()?;

        Ok(Pairing {
            tenant_id,
            peer: issuer,
            created_at: now,
        })
    }

    pub fn list_pairings(&self, device_id: &str) -> Result<Vec<Pairing>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT p.tenant_id,
                    d.id, d.device_name, d.platform, d.model, d.os_version, d.created_at, d.last_seen,
                    p.created_at
             FROM pairings p
             JOIN devices d
               ON d.id = CASE
                    WHEN p.windows_device_id = ?1 THEN p.android_device_id
                    ELSE p.windows_device_id
                  END
             WHERE p.revoked_at IS NULL
               AND (p.windows_device_id = ?1 OR p.android_device_id = ?1)
             ORDER BY p.created_at DESC",
        )?;

        let rows = stmt.query_map(params![device_id], |row| {
            Ok(Pairing {
                tenant_id: row.get(0)?,
                peer: Device {
                    id: row.get(1)?,
                    device_name: row.get(2)?,
                    platform: row.get(3)?,
                    model: row.get(4)?,
                    os_version: row.get(5)?,
                    created_at: row.get(6)?,
                    last_seen: row.get(7)?,
                },
                created_at: row.get(8)?,
            })
        })?;

        let mut pairings = Vec::new();
        for row in rows {
            pairings.push(row?);
        }
        Ok(pairings)
    }

    pub fn route_between(&self, tenant_id: &str, from: &str, to: &str) -> Result<Option<Route>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT tenant_id, windows_device_id, android_device_id
             FROM pairings
             WHERE tenant_id = ?1
               AND revoked_at IS NULL
               AND ((windows_device_id = ?2 AND android_device_id = ?3)
                    OR (android_device_id = ?2 AND windows_device_id = ?3))",
            params![tenant_id, from, to],
            |_row| Ok(Route),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn revoke_pairing(&self, tenant_id: &str, device_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE pairings SET revoked_at = ?1
             WHERE tenant_id = ?2
               AND revoked_at IS NULL
               AND (windows_device_id = ?3 OR android_device_id = ?3)",
            params![now_ms(), tenant_id, device_id],
        )?;
        if changed == 0 {
            return Err(RelayError::not_found(
                ERROR_INVALID_PAIRING_CODE,
                "pairing was not found",
            ));
        }
        Ok(())
    }

    pub fn insert_command_and_enqueue(
        &self,
        envelope: &CommandEnvelope,
        envelope_json: &str,
        max_pending_messages: i64,
    ) -> Result<CommandRecord> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let existing = tx
            .query_row(
                "SELECT from_device_id, to_device_id, envelope_json, state
                 FROM commands WHERE command_id = ?1",
                params![envelope.command_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;

        if let Some((from_device_id, to_device_id, existing_json, state)) = existing {
            if from_device_id != envelope.from_device_id || to_device_id != envelope.to_device_id {
                return Err(RelayError::conflict(
                    "502001",
                    "command_id was already used for another route",
                ));
            }
            return Ok(CommandRecord {
                inserted: false,
                state,
                envelope_json: existing_json,
            });
        }

        let state = command_state_name(envelope.state.clone().unwrap_or(CommandState::Sent));
        let now = now_ms();
        tx.execute(
            "INSERT INTO commands
             (command_id, tenant_id, from_device_id, to_device_id, action_type, envelope_json, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                envelope.command_id,
                envelope.tenant_id,
                envelope.from_device_id,
                envelope.to_device_id,
                envelope.action_type,
                envelope_json,
                state,
                now
            ],
        )?;
        enqueue_message(
            &tx,
            &envelope.id,
            &envelope.to_device_id,
            envelope_json,
            now,
        )?;
        prune_messages(&tx, &envelope.to_device_id, max_pending_messages)?;
        tx.commit()?;

        Ok(CommandRecord {
            inserted: true,
            state,
            envelope_json: envelope_json.to_string(),
        })
    }

    pub fn enqueue_message(
        &self,
        id: &str,
        target_device_id: &str,
        envelope_json: &str,
        max_pending_messages: i64,
    ) -> Result<()> {
        let conn = self.conn.lock();
        enqueue_message(&conn, id, target_device_id, envelope_json, now_ms())?;
        prune_messages(&conn, target_device_id, max_pending_messages)?;
        Ok(())
    }

    pub fn pending_messages(
        &self,
        target_device_id: &str,
        limit: i64,
    ) -> Result<Vec<PendingMessage>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, envelope_json
             FROM relay_messages
             WHERE target_device_id = ?1 AND delivered_at IS NULL
             ORDER BY created_at ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![target_device_id, limit], |row| {
            Ok(PendingMessage {
                id: row.get(0)?,
                envelope_json: row.get(1)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn mark_message_delivered(&self, message_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE relay_messages SET delivered_at = ?1
             WHERE id = ?2 AND delivered_at IS NULL",
            params![now_ms(), message_id],
        )?;
        Ok(())
    }

    pub fn update_command_state(
        &self,
        command_id: &str,
        from_device_id: &str,
        next_state: &CommandState,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        let current = conn
            .query_row(
                "SELECT state FROM commands
                 WHERE command_id = ?1 AND to_device_id = ?2",
                params![command_id, from_device_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(current) = current else {
            return Ok(false);
        };
        let next = command_state_name(next_state.clone());
        if state_rank(&current) >= state_rank(&next) {
            return Ok(false);
        }
        conn.execute(
            "UPDATE commands SET state = ?1, updated_at = ?2 WHERE command_id = ?3",
            params![next, now_ms(), command_id],
        )?;
        Ok(true)
    }

    #[cfg(test)]
    pub fn command_state(&self, command_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT state FROM commands WHERE command_id = ?1",
            params![command_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    Ok(())
}

fn get_device(conn: &Connection, device_id: &str) -> Result<Device> {
    conn.query_row(
        "SELECT id, device_name, platform, model, os_version, created_at, last_seen
         FROM devices WHERE id = ?1",
        params![device_id],
        |row| {
            Ok(Device {
                id: row.get(0)?,
                device_name: row.get(1)?,
                platform: row.get(2)?,
                model: row.get(3)?,
                os_version: row.get(4)?,
                created_at: row.get(5)?,
                last_seen: row.get(6)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| RelayError::not_found("501002", "device was not found"))
}

fn find_pair_by_devices(
    conn: &Connection,
    windows_device_id: &str,
    android_device_id: &str,
) -> Result<Option<PairingStored>> {
    conn.query_row(
        "SELECT tenant_id, created_at FROM pairings
         WHERE windows_device_id = ?1 AND android_device_id = ?2 AND revoked_at IS NULL",
        params![windows_device_id, android_device_id],
        |row| {
            Ok(PairingStored {
                tenant_id: row.get(0)?,
                created_at: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn enqueue_message(
    conn: &Connection,
    id: &str,
    target_device_id: &str,
    envelope_json: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relay_messages (id, target_device_id, envelope_json, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, target_device_id, envelope_json, now],
    )?;
    conn.execute(
        "DELETE FROM relay_messages
         WHERE delivered_at IS NOT NULL AND delivered_at < ?1",
        params![now - 7 * 24 * 60 * 60 * 1_000],
    )?;
    Ok(())
}

fn prune_messages(
    conn: &Connection,
    target_device_id: &str,
    max_pending_messages: i64,
) -> Result<()> {
    if max_pending_messages <= 0 {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM relay_messages
         WHERE id IN (
             SELECT id FROM relay_messages
             WHERE target_device_id = ?1 AND delivered_at IS NULL
             ORDER BY created_at DESC
             LIMIT -1 OFFSET ?2
         )",
        params![target_device_id, max_pending_messages],
    )?;
    Ok(())
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && bool::from(left.ct_eq(right))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn command_state_name(state: CommandState) -> String {
    match state {
        CommandState::Sent => "sent",
        CommandState::Delivered => "delivered",
        CommandState::Running => "running",
        CommandState::Done => "done",
        CommandState::Failed => "failed",
    }
    .to_string()
}

fn state_rank(state: &str) -> u8 {
    match state {
        "sent" => 1,
        "delivered" => 2,
        "running" => 3,
        "done" | "failed" => 4,
        _ => 0,
    }
}

#[derive(Debug)]
struct PairingStored {
    tenant_id: String,
    created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use localmind_shared_contract::EnvelopeType;

    fn db() -> Db {
        Db::open_in_memory().expect("in-memory database")
    }

    #[test]
    fn authentication_round_trip() {
        let db = db();
        let (device, token) = db
            .register_device("desktop", "windows", "pc", "Windows 11")
            .unwrap();
        let authenticated = db.authenticate(&device.id, &token).unwrap();
        assert_eq!(authenticated.id, device.id);
        assert!(db.authenticate(&device.id, "wrong").is_err());
    }

    #[test]
    fn pairing_and_command_idempotency() {
        let db = db();
        let (windows, _) = db
            .register_device("desktop", "windows", "pc", "Windows 11")
            .unwrap();
        let (android, _) = db
            .register_device("phone", "android", "Pixel", "Android 15")
            .unwrap();
        let (code, _) = db.issue_pairing_code(&windows.id, 300).unwrap();
        let pairing = db.claim_pairing_code(&android.id, &code).unwrap();
        assert_eq!(pairing.peer.id, windows.id);

        let mut envelope = CommandEnvelope::new_heartbeat(&windows.id);
        envelope.envelope_type = EnvelopeType::Command;
        envelope.to_device_id = android.id.clone();
        envelope.tenant_id = pairing.tenant_id.clone();
        envelope.command_id = Uuid::new_v4().to_string();
        envelope.action_type = "device.ping".to_string();
        envelope.intent_text = "ping".to_string();
        envelope.state = Some(CommandState::Sent);
        let json = serde_json::to_string(&envelope).unwrap();

        let first = db
            .insert_command_and_enqueue(&envelope, &json, 500)
            .unwrap();
        let second = db
            .insert_command_and_enqueue(&envelope, &json, 500)
            .unwrap();
        assert!(first.inserted);
        assert!(!second.inserted);
        assert_eq!(db.pending_messages(&android.id, 10).unwrap().len(), 1);
    }

    #[test]
    fn state_only_moves_forward() {
        let db = db();
        let (windows, _) = db
            .register_device("desktop", "windows", "pc", "Windows 11")
            .unwrap();
        let (android, _) = db
            .register_device("phone", "android", "Pixel", "Android 15")
            .unwrap();
        let (code, _) = db.issue_pairing_code(&windows.id, 300).unwrap();
        let pairing = db.claim_pairing_code(&android.id, &code).unwrap();

        let mut envelope = CommandEnvelope::new_heartbeat(&windows.id);
        envelope.envelope_type = EnvelopeType::Command;
        envelope.to_device_id = android.id.clone();
        envelope.tenant_id = pairing.tenant_id.clone();
        envelope.command_id = Uuid::new_v4().to_string();
        envelope.action_type = "device.ping".to_string();
        envelope.intent_text = "ping".to_string();
        envelope.state = Some(CommandState::Sent);
        let json = serde_json::to_string(&envelope).unwrap();
        db.insert_command_and_enqueue(&envelope, &json, 500)
            .unwrap();

        assert!(db
            .update_command_state(&envelope.command_id, &android.id, &CommandState::Running)
            .unwrap());
        assert!(!db
            .update_command_state(&envelope.command_id, &android.id, &CommandState::Sent)
            .unwrap());
        assert_eq!(
            db.command_state(&envelope.command_id).unwrap().as_deref(),
            Some("running")
        );
    }
}
