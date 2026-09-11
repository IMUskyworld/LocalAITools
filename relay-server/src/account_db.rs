use argon2::{
    password_hash::{rand_core::OsRng as PasswordOsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use uuid::Uuid;

use crate::db::{get_device, hash_token, now_ms, random_token, Db, Device};
use crate::error::{RelayError, Result};
use crate::protocol::{
    ERROR_ACCOUNT_DISABLED, ERROR_DEVICE_ALREADY_ENROLLED, ERROR_DEVICE_NOT_ENROLLED,
    ERROR_EMAIL_ALREADY_REGISTERED, ERROR_INVALID_CREDENTIALS, ERROR_PAIRING_REQUEST_CONFLICT,
    ERROR_PAIRING_REQUEST_EXPIRED, ERROR_TOKEN_INVALID, ERROR_UNAUTHORIZED,
};

const USER_ACTIVE: &str = "active";
const REQUEST_PENDING: &str = "pending";

const SCHEMA_V2: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL COLLATE NOCASE UNIQUE,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS user_credentials (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS auth_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT REFERENCES devices(id) ON DELETE SET NULL,
    access_token_hash TEXT NOT NULL UNIQUE,
    refresh_token_hash TEXT NOT NULL UNIQUE,
    access_expires_at INTEGER NOT NULL,
    refresh_expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    revoked_at INTEGER,
    replaced_by_session_id TEXT REFERENCES auth_sessions(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_user
    ON auth_sessions(user_id, revoked_at, refresh_expires_at);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_refresh
    ON auth_sessions(refresh_token_hash, revoked_at);

CREATE TABLE IF NOT EXISTS device_enrollments (
    device_id TEXT PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_device_enrollments_account
    ON device_enrollments(account_id, created_at);

CREATE TABLE IF NOT EXISTS pairing_requests (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    requester_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    target_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    permissions_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected', 'expired')),
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    resolved_at INTEGER,
    resolved_by_device_id TEXT REFERENCES devices(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_pairing_requests_one_pending
    ON pairing_requests(account_id, requester_device_id, target_device_id)
    WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_pairing_requests_target
    ON pairing_requests(target_device_id, status, expires_at);
CREATE INDEX IF NOT EXISTS idx_pairing_requests_requester
    ON pairing_requests(requester_device_id, status, expires_at);

CREATE TABLE IF NOT EXISTS device_pairings (
    tenant_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    controller_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    target_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    permissions_json TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL,
    approved_at INTEGER NOT NULL,
    revoked_at INTEGER,
    UNIQUE (controller_device_id, target_device_id)
);

CREATE INDEX IF NOT EXISTS idx_device_pairings_controller
    ON device_pairings(controller_device_id, revoked_at);
CREATE INDEX IF NOT EXISTS idx_device_pairings_target
    ON device_pairings(target_device_id, revoked_at);

CREATE TABLE IF NOT EXISTS auth_audit_log (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    device_id TEXT REFERENCES devices(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_audit_user_time
    ON auth_audit_log(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_auth_audit_device_time
    ON auth_audit_log(device_id, created_at DESC);
"#;

#[derive(Clone, Debug, Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: i64,
    pub refresh_expires_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AccountDevice {
    #[serde(flatten)]
    pub device: Device,
    pub enrolled_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PairingRequest {
    pub id: String,
    pub requester_device_id: String,
    pub target_device_id: String,
    pub permissions: Vec<String>,
    pub status: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub resolved_at: Option<i64>,
    pub resolved_by_device_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DevicePairing {
    pub tenant_id: String,
    pub controller_device_id: String,
    pub target_device_id: String,
    pub permissions: Vec<String>,
    pub created_at: i64,
    pub approved_at: i64,
}

pub(crate) fn migrate_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_V2)?;
    Ok(())
}

impl Db {
    pub fn register_user(
        &self,
        email: &str,
        display_name: &str,
        password: &str,
        access_ttl_seconds: i64,
        refresh_ttl_seconds: i64,
    ) -> Result<(User, AuthTokens)> {
        let email = normalize_email(email)?;
        let display_name = display_name.trim();
        if display_name.is_empty() || display_name.chars().count() > 80 {
            return Err(RelayError::bad_request(
                ERROR_UNAUTHORIZED,
                "display_name must contain between 1 and 80 characters",
            ));
        }
        validate_password(password)?;
        let password_hash = hash_password(password)?;
        let now = now_ms();
        let user = User {
            id: Uuid::new_v4().to_string(),
            email,
            display_name: display_name.to_string(),
            status: USER_ACTIVE.to_string(),
            created_at: now,
            updated_at: now,
        };
        let session = new_session(access_ttl_seconds, refresh_ttl_seconds);

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let exists = tx
            .query_row(
                "SELECT 1 FROM users WHERE email = ?1 COLLATE NOCASE",
                params![user.email],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if exists {
            return Err(RelayError::conflict(
                ERROR_EMAIL_ALREADY_REGISTERED,
                "email is already registered",
            ));
        }
        tx.execute(
            "INSERT INTO users (id, email, display_name, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                user.id,
                user.email,
                user.display_name,
                user.status,
                user.created_at
            ],
        )?;
        tx.execute(
            "INSERT INTO user_credentials (user_id, password_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)",
            params![user.id, password_hash, now],
        )?;
        insert_session(&tx, &user.id, None, &session, now)?;
        insert_audit(&tx, Some(&user.id), None, "account.registered", "{}", now)?;
        tx.commit()?;
        Ok((user, session.into_tokens()))
    }

    pub fn login_user(
        &self,
        email: &str,
        password: &str,
        device_id: Option<&str>,
        access_ttl_seconds: i64,
        refresh_ttl_seconds: i64,
    ) -> Result<(User, AuthTokens)> {
        let email = normalize_email(email)?;
        let row = {
            let conn = self.conn.lock();
            conn.query_row(
                "SELECT u.id, u.email, u.display_name, u.status, u.created_at, u.updated_at,
                        c.password_hash
                 FROM users u
                 JOIN user_credentials c ON c.user_id = u.id
                 WHERE u.email = ?1 COLLATE NOCASE",
                params![email],
                |row| {
                    Ok((
                        User {
                            id: row.get(0)?,
                            email: row.get(1)?,
                            display_name: row.get(2)?,
                            status: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        },
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?
        };

        let Some((user, password_hash)) = row else {
            return Err(RelayError::unauthorized(
                ERROR_INVALID_CREDENTIALS,
                "invalid email or password",
            ));
        };
        if user.status != USER_ACTIVE {
            return Err(RelayError::forbidden(
                ERROR_ACCOUNT_DISABLED,
                "account is disabled",
            ));
        }
        if !verify_password(password, &password_hash)? {
            return Err(RelayError::unauthorized(
                ERROR_INVALID_CREDENTIALS,
                "invalid email or password",
            ));
        }

        let now = now_ms();
        let session = new_session(access_ttl_seconds, refresh_ttl_seconds);
        let conn = self.conn.lock();
        insert_session(&conn, &user.id, device_id, &session, now)?;
        insert_audit(
            &conn,
            Some(&user.id),
            device_id,
            "account.logged_in",
            "{}",
            now,
        )?;
        Ok((user, session.into_tokens()))
    }

    pub fn refresh_session(
        &self,
        refresh_token: &str,
        access_ttl_seconds: i64,
        refresh_ttl_seconds: i64,
    ) -> Result<(User, AuthTokens)> {
        let now = now_ms();
        let refresh_hash = hash_token(refresh_token);
        let next = new_session(access_ttl_seconds, refresh_ttl_seconds);
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let row = tx
            .query_row(
                "SELECT s.id, s.device_id, s.refresh_expires_at, s.revoked_at,
                        u.id, u.email, u.display_name, u.status, u.created_at, u.updated_at
                 FROM auth_sessions s
                 JOIN users u ON u.id = s.user_id
                 WHERE s.refresh_token_hash = ?1",
                params![refresh_hash],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        User {
                            id: row.get(4)?,
                            email: row.get(5)?,
                            display_name: row.get(6)?,
                            status: row.get(7)?,
                            created_at: row.get(8)?,
                            updated_at: row.get(9)?,
                        },
                    ))
                },
            )
            .optional()?;

        let Some((old_session_id, device_id, refresh_expires_at, revoked_at, user)) = row else {
            return Err(RelayError::unauthorized(
                ERROR_TOKEN_INVALID,
                "refresh token is invalid or expired",
            ));
        };
        if revoked_at.is_some() || refresh_expires_at <= now {
            return Err(RelayError::unauthorized(
                ERROR_TOKEN_INVALID,
                "refresh token is invalid or expired",
            ));
        }
        if user.status != USER_ACTIVE {
            return Err(RelayError::forbidden(
                ERROR_ACCOUNT_DISABLED,
                "account is disabled",
            ));
        }

        insert_session(&tx, &user.id, device_id.as_deref(), &next, now)?;
        let changed = tx.execute(
            "UPDATE auth_sessions
             SET revoked_at = ?1, replaced_by_session_id = ?2
             WHERE id = ?3 AND revoked_at IS NULL",
            params![now, next.id, old_session_id],
        )?;
        if changed != 1 {
            return Err(RelayError::unauthorized(
                ERROR_TOKEN_INVALID,
                "refresh token has already been rotated",
            ));
        }
        insert_audit(
            &tx,
            Some(&user.id),
            device_id.as_deref(),
            "account.session_refreshed",
            "{}",
            now,
        )?;
        tx.commit()?;
        Ok((user, next.into_tokens()))
    }

    pub fn logout_session(&self, refresh_token: &str) -> Result<()> {
        let refresh_hash = hash_token(refresh_token);
        let now = now_ms();
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE auth_sessions
             SET revoked_at = ?1
             WHERE refresh_token_hash = ?2 AND revoked_at IS NULL",
            params![now, refresh_hash],
        )?;
        Ok(())
    }

    pub fn current_user_by_access_token(&self, access_token: &str) -> Result<User> {
        let now = now_ms();
        let access_hash = hash_token(access_token);
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "SELECT u.id, u.email, u.display_name, u.status, u.created_at, u.updated_at,
                        s.id, s.access_expires_at, s.revoked_at
                 FROM auth_sessions s
                 JOIN users u ON u.id = s.user_id
                 WHERE s.access_token_hash = ?1",
                params![access_hash],
                |row| {
                    Ok((
                        User {
                            id: row.get(0)?,
                            email: row.get(1)?,
                            display_name: row.get(2)?,
                            status: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        },
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                    ))
                },
            )
            .optional()?;
        let Some((user, session_id, access_expires_at, revoked_at)) = row else {
            return Err(RelayError::unauthorized(
                ERROR_TOKEN_INVALID,
                "access token is invalid or expired",
            ));
        };
        if revoked_at.is_some() || access_expires_at <= now {
            return Err(RelayError::unauthorized(
                ERROR_TOKEN_INVALID,
                "access token is invalid or expired",
            ));
        }
        if user.status != USER_ACTIVE {
            return Err(RelayError::forbidden(
                ERROR_ACCOUNT_DISABLED,
                "account is disabled",
            ));
        }
        conn.execute(
            "UPDATE auth_sessions SET last_used_at = ?1 WHERE id = ?2",
            params![now, session_id],
        )?;
        Ok(user)
    }

    pub fn enroll_device(
        &self,
        account_id: &str,
        device_id: &str,
        device_token: &str,
    ) -> Result<AccountDevice> {
        let device = self.authenticate(device_id, device_token)?;
        let now = now_ms();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let existing_account = tx
            .query_row(
                "SELECT account_id FROM device_enrollments WHERE device_id = ?1",
                params![device_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(existing_account) = existing_account {
            if existing_account != account_id {
                return Err(RelayError::conflict(
                    ERROR_DEVICE_ALREADY_ENROLLED,
                    "device is already enrolled by another account",
                ));
            }
        } else {
            tx.execute(
                "INSERT INTO device_enrollments (device_id, account_id, created_at)
                 VALUES (?1, ?2, ?3)",
                params![device_id, account_id, now],
            )?;
        }
        insert_audit(
            &tx,
            Some(account_id),
            Some(device_id),
            "device.enrolled",
            "{}",
            now,
        )?;
        tx.commit()?;
        Ok(AccountDevice {
            device,
            enrolled_at: now,
        })
    }

    pub fn ensure_account_device(
        &self,
        account_id: &str,
        device_id: &str,
    ) -> Result<AccountDevice> {
        let conn = self.conn.lock();
        ensure_enrollment_conn(&conn, account_id, device_id)?;
        let enrolled_at = conn.query_row(
            "SELECT created_at FROM device_enrollments WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id],
            |row| row.get(0),
        )?;
        let device = get_device(&conn, device_id)?;
        Ok(AccountDevice {
            device,
            enrolled_at,
        })
    }

    pub fn list_account_devices(&self, account_id: &str) -> Result<Vec<AccountDevice>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT d.id, d.device_name, d.platform, d.model, d.os_version, d.created_at, d.last_seen,
                    e.created_at
             FROM device_enrollments e
             JOIN devices d ON d.id = e.device_id
             WHERE e.account_id = ?1
             ORDER BY e.created_at ASC",
        )?;
        let rows = stmt.query_map(params![account_id], |row| {
            Ok(AccountDevice {
                device: Device {
                    id: row.get(0)?,
                    device_name: row.get(1)?,
                    platform: row.get(2)?,
                    model: row.get(3)?,
                    os_version: row.get(4)?,
                    created_at: row.get(5)?,
                    last_seen: row.get(6)?,
                },
                enrolled_at: row.get(7)?,
            })
        })?;
        collect_rows(rows)
    }

    pub fn remove_account_device(
        &self,
        account_id: &str,
        actor_device_id: &str,
        target_device_id: &str,
    ) -> Result<()> {
        let now = now_ms();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        ensure_enrollment_tx(&tx, account_id, actor_device_id)?;
        ensure_enrollment_tx(&tx, account_id, target_device_id)?;
        tx.execute(
            "DELETE FROM device_enrollments WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, target_device_id],
        )?;
        tx.execute(
            "UPDATE device_pairings SET revoked_at = ?1
             WHERE account_id = ?2 AND revoked_at IS NULL
               AND (controller_device_id = ?3 OR target_device_id = ?3)",
            params![now, account_id, target_device_id],
        )?;
        tx.execute(
            "UPDATE pairing_requests SET status = 'rejected', resolved_at = ?1,
                    resolved_by_device_id = ?2
             WHERE account_id = ?3 AND status = 'pending'
               AND (requester_device_id = ?2 OR target_device_id = ?2)",
            params![now, target_device_id, account_id],
        )?;
        tx.execute(
            "UPDATE auth_sessions SET revoked_at = ?1
             WHERE user_id = ?2 AND device_id = ?3 AND revoked_at IS NULL",
            params![now, account_id, target_device_id],
        )?;
        insert_audit(
            &tx,
            Some(account_id),
            Some(actor_device_id),
            "device.removed",
            &format!(
                "{{\"target_device_id\":\"{}\"}}",
                json_escape(target_device_id)
            ),
            now,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn create_pairing_request(
        &self,
        account_id: &str,
        requester_device_id: &str,
        target_device_id: &str,
        permissions: &[String],
        ttl_seconds: i64,
    ) -> Result<PairingRequest> {
        if requester_device_id == target_device_id {
            return Err(RelayError::bad_request(
                ERROR_UNAUTHORIZED,
                "a device cannot request control over itself",
            ));
        }
        let now = now_ms();
        let expires_at = now + ttl_seconds.max(60) * 1_000;
        let permissions_json = serde_json::to_string(permissions)?;
        let id = Uuid::new_v4().to_string();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let requester = ensure_enrollment_tx(&tx, account_id, requester_device_id)?;
        let target = ensure_enrollment_tx(&tx, account_id, target_device_id)?;
        if requester.platform != "android" || target.platform != "windows" {
            return Err(RelayError::forbidden(
                ERROR_UNAUTHORIZED,
                "control requests must go from an Android device to a Windows device",
            ));
        }
        expire_pairing_requests_tx(&tx, now)?;
        let existing = tx
            .query_row(
                "SELECT id FROM pairing_requests
                 WHERE account_id = ?1 AND requester_device_id = ?2
                   AND target_device_id = ?3 AND status = 'pending'",
                params![account_id, requester_device_id, target_device_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if existing.is_some() {
            return Err(RelayError::conflict(
                ERROR_PAIRING_REQUEST_CONFLICT,
                "a pending control request already exists",
            ));
        }
        tx.execute(
            "INSERT INTO pairing_requests
             (id, account_id, requester_device_id, target_device_id, permissions_json,
              status, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7)",
            params![
                id,
                account_id,
                requester_device_id,
                target_device_id,
                permissions_json,
                now,
                expires_at
            ],
        )?;
        insert_audit(
            &tx,
            Some(account_id),
            Some(requester_device_id),
            "control.requested",
            &format!(
                "{{\"request_id\":\"{}\",\"target_device_id\":\"{}\"}}",
                json_escape(&id),
                json_escape(target_device_id)
            ),
            now,
        )?;
        tx.commit()?;
        Ok(PairingRequest {
            id,
            requester_device_id: requester_device_id.to_string(),
            target_device_id: target_device_id.to_string(),
            permissions: permissions.to_vec(),
            status: REQUEST_PENDING.to_string(),
            created_at: now,
            expires_at,
            resolved_at: None,
            resolved_by_device_id: None,
        })
    }

    pub fn list_pairing_requests(
        &self,
        account_id: &str,
        device_id: &str,
    ) -> Result<Vec<PairingRequest>> {
        let now = now_ms();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        ensure_enrollment_tx(&tx, account_id, device_id)?;
        expire_pairing_requests_tx(&tx, now)?;
        let rows = {
            let mut stmt = tx.prepare(
                "SELECT id, requester_device_id, target_device_id, permissions_json, status,
                        created_at, expires_at, resolved_at, resolved_by_device_id
                 FROM pairing_requests
                 WHERE account_id = ?1 AND (requester_device_id = ?2 OR target_device_id = ?2)
                 ORDER BY created_at DESC",
            )?;
            let mapped = stmt.query_map(params![account_id, device_id], map_pairing_request)?;
            collect_rows(mapped)?
        };
        tx.commit()?;
        Ok(rows)
    }

    pub fn approve_pairing_request(
        &self,
        account_id: &str,
        target_device_id: &str,
        request_id: &str,
    ) -> Result<DevicePairing> {
        let now = now_ms();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        ensure_enrollment_tx(&tx, account_id, target_device_id)?;
        let request = get_pairing_request_tx(&tx, request_id)?;
        if request.account_id != account_id || request.target_device_id != target_device_id {
            return Err(RelayError::forbidden(
                ERROR_UNAUTHORIZED,
                "only the target Windows device can approve this request",
            ));
        }
        if request.status == REQUEST_PENDING && request.expires_at <= now {
            tx.execute(
                "UPDATE pairing_requests SET status = 'expired', resolved_at = ?1
                 WHERE id = ?2 AND status = 'pending'",
                params![now, request_id],
            )?;
            return Err(RelayError::bad_request(
                ERROR_PAIRING_REQUEST_EXPIRED,
                "pairing request has expired",
            ));
        }
        if request.status != REQUEST_PENDING {
            return Err(RelayError::conflict(
                ERROR_PAIRING_REQUEST_CONFLICT,
                "pairing request is no longer pending",
            ));
        }
        let existing = tx
            .query_row(
                "SELECT tenant_id, created_at FROM device_pairings
                 WHERE controller_device_id = ?1 AND target_device_id = ?2",
                params![request.requester_device_id, request.target_device_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let (tenant_id, created_at) = if let Some((tenant_id, created_at)) = existing {
            tx.execute(
                "UPDATE device_pairings
                 SET account_id = ?1, permissions_json = ?2, approved_at = ?3, revoked_at = NULL
                 WHERE tenant_id = ?4",
                params![account_id, request.permissions_json, now, tenant_id],
            )?;
            (tenant_id, created_at)
        } else {
            let tenant_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO device_pairings
                 (tenant_id, account_id, controller_device_id, target_device_id,
                  permissions_json, created_at, approved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    tenant_id,
                    account_id,
                    request.requester_device_id,
                    request.target_device_id,
                    request.permissions_json,
                    now
                ],
            )?;
            (tenant_id, now)
        };
        tx.execute(
            "UPDATE pairing_requests
             SET status = 'approved', resolved_at = ?1, resolved_by_device_id = ?2
             WHERE id = ?3 AND status = 'pending'",
            params![now, target_device_id, request_id],
        )?;
        insert_audit(
            &tx,
            Some(account_id),
            Some(target_device_id),
            "control.approved",
            &format!(
                "{{\"request_id\":\"{}\",\"tenant_id\":\"{}\"}}",
                json_escape(request_id),
                json_escape(&tenant_id)
            ),
            now,
        )?;
        tx.commit()?;
        Ok(DevicePairing {
            tenant_id,
            controller_device_id: request.requester_device_id,
            target_device_id: request.target_device_id,
            permissions: serde_json::from_str(&request.permissions_json)?,
            created_at,
            approved_at: now,
        })
    }

    pub fn reject_pairing_request(
        &self,
        account_id: &str,
        target_device_id: &str,
        request_id: &str,
    ) -> Result<()> {
        let now = now_ms();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        ensure_enrollment_tx(&tx, account_id, target_device_id)?;
        let request = get_pairing_request_tx(&tx, request_id)?;
        if request.account_id != account_id || request.target_device_id != target_device_id {
            return Err(RelayError::forbidden(
                ERROR_UNAUTHORIZED,
                "only the target Windows device can reject this request",
            ));
        }
        if request.status == REQUEST_PENDING && request.expires_at <= now {
            tx.execute(
                "UPDATE pairing_requests SET status = 'expired', resolved_at = ?1
                 WHERE id = ?2 AND status = 'pending'",
                params![now, request_id],
            )?;
            return Err(RelayError::bad_request(
                ERROR_PAIRING_REQUEST_EXPIRED,
                "pairing request has expired",
            ));
        }
        if request.status != REQUEST_PENDING {
            return Err(RelayError::conflict(
                ERROR_PAIRING_REQUEST_CONFLICT,
                "pairing request is no longer pending",
            ));
        }
        tx.execute(
            "UPDATE pairing_requests
             SET status = 'rejected', resolved_at = ?1, resolved_by_device_id = ?2
             WHERE id = ?3 AND status = 'pending'",
            params![now, target_device_id, request_id],
        )?;
        insert_audit(
            &tx,
            Some(account_id),
            Some(target_device_id),
            "control.rejected",
            &format!("{{\"request_id\":\"{}\"}}", json_escape(request_id)),
            now,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_control_pairings(
        &self,
        account_id: &str,
        device_id: &str,
    ) -> Result<Vec<DevicePairing>> {
        let conn = self.conn.lock();
        ensure_enrollment_conn(&conn, account_id, device_id)?;
        let mut stmt = conn.prepare(
            "SELECT tenant_id, controller_device_id, target_device_id, permissions_json,
                    created_at, approved_at
             FROM device_pairings
             WHERE account_id = ?1 AND revoked_at IS NULL
               AND (controller_device_id = ?2 OR target_device_id = ?2)
             ORDER BY approved_at DESC",
        )?;
        let rows = stmt.query_map(params![account_id, device_id], map_device_pairing)?;
        collect_rows(rows)
    }

    pub fn revoke_control_pairing(
        &self,
        account_id: &str,
        device_id: &str,
        tenant_id: &str,
    ) -> Result<()> {
        let now = now_ms();
        let conn = self.conn.lock();
        ensure_enrollment_conn(&conn, account_id, device_id)?;
        let changed = conn.execute(
            "UPDATE device_pairings SET revoked_at = ?1
             WHERE tenant_id = ?2 AND account_id = ?3 AND revoked_at IS NULL
               AND (controller_device_id = ?4 OR target_device_id = ?4)",
            params![now, tenant_id, account_id, device_id],
        )?;
        if changed == 0 {
            return Err(RelayError::not_found(
                ERROR_DEVICE_NOT_ENROLLED,
                "control pairing was not found",
            ));
        }
        insert_audit(
            &conn,
            Some(account_id),
            Some(device_id),
            "control.revoked",
            &format!("{{\"tenant_id\":\"{}\"}}", json_escape(tenant_id)),
            now,
        )?;
        Ok(())
    }

    pub fn append_audit(
        &self,
        user_id: Option<&str>,
        device_id: Option<&str>,
        event_type: &str,
        metadata_json: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        insert_audit(
            &conn,
            user_id,
            device_id,
            event_type,
            metadata_json,
            now_ms(),
        )
    }
}

struct NewSession {
    id: String,
    access_token: String,
    refresh_token: String,
    access_token_hash: String,
    refresh_token_hash: String,
    access_expires_at: i64,
    refresh_expires_at: i64,
}

impl NewSession {
    fn into_tokens(self) -> AuthTokens {
        AuthTokens {
            access_token: self.access_token,
            refresh_token: self.refresh_token,
            access_expires_at: self.access_expires_at,
            refresh_expires_at: self.refresh_expires_at,
        }
    }
}

struct StoredPairingRequest {
    account_id: String,
    requester_device_id: String,
    target_device_id: String,
    permissions_json: String,
    status: String,
    expires_at: i64,
}

fn new_session(access_ttl_seconds: i64, refresh_ttl_seconds: i64) -> NewSession {
    let now = now_ms();
    let access_token = random_token();
    let refresh_token = random_token();
    NewSession {
        id: Uuid::new_v4().to_string(),
        access_token_hash: hash_token(&access_token),
        refresh_token_hash: hash_token(&refresh_token),
        access_token,
        refresh_token,
        access_expires_at: now + access_ttl_seconds.max(60) * 1_000,
        refresh_expires_at: now + refresh_ttl_seconds.max(300) * 1_000,
    }
}

fn insert_session(
    conn: &Connection,
    user_id: &str,
    device_id: Option<&str>,
    session: &NewSession,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO auth_sessions
         (id, user_id, device_id, access_token_hash, refresh_token_hash,
          access_expires_at, refresh_expires_at, created_at, last_used_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            session.id,
            user_id,
            device_id,
            session.access_token_hash,
            session.refresh_token_hash,
            session.access_expires_at,
            session.refresh_expires_at,
            now
        ],
    )?;
    Ok(())
}

fn normalize_email(email: &str) -> Result<String> {
    let email = email.trim().to_ascii_lowercase();
    let valid = email.len() <= 254
        && email
            .split_once('@')
            .map(|(local, domain)| {
                !local.is_empty()
                    && !domain.is_empty()
                    && !domain.starts_with('.')
                    && domain.contains('.')
            })
            .unwrap_or(false);
    if !valid {
        return Err(RelayError::bad_request(
            ERROR_UNAUTHORIZED,
            "email address is invalid",
        ));
    }
    Ok(email)
}

fn validate_password(password: &str) -> Result<()> {
    let count = password.chars().count();
    if !(8..=128).contains(&count) {
        return Err(RelayError::bad_request(
            ERROR_UNAUTHORIZED,
            "password must contain between 8 and 128 characters",
        ));
    }
    Ok(())
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut PasswordOsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| RelayError::Internal(format!("failed to hash password: {err}")))
}

fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(password_hash)
        .map_err(|err| RelayError::Internal(format!("stored password hash is invalid: {err}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn ensure_enrollment_tx(conn: &Connection, account_id: &str, device_id: &str) -> Result<Device> {
    ensure_enrollment_conn(conn, account_id, device_id)?;
    get_device(conn, device_id)
}

fn ensure_enrollment_conn(conn: &Connection, account_id: &str, device_id: &str) -> Result<()> {
    let enrolled = conn
        .query_row(
            "SELECT 1 FROM device_enrollments WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !enrolled {
        return Err(RelayError::forbidden(
            ERROR_DEVICE_NOT_ENROLLED,
            "device is not enrolled in this account",
        ));
    }
    Ok(())
}

fn get_pairing_request_tx(conn: &Connection, request_id: &str) -> Result<StoredPairingRequest> {
    conn.query_row(
        "SELECT account_id, requester_device_id, target_device_id, permissions_json,
                status, expires_at
         FROM pairing_requests WHERE id = ?1",
        params![request_id],
        |row| {
            Ok(StoredPairingRequest {
                account_id: row.get(0)?,
                requester_device_id: row.get(1)?,
                target_device_id: row.get(2)?,
                permissions_json: row.get(3)?,
                status: row.get(4)?,
                expires_at: row.get(5)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| {
        RelayError::not_found(
            ERROR_PAIRING_REQUEST_CONFLICT,
            "pairing request was not found",
        )
    })
}

fn expire_pairing_requests_tx(conn: &Connection, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE pairing_requests SET status = 'expired', resolved_at = ?1
         WHERE status = 'pending' AND expires_at <= ?1",
        params![now],
    )?;
    Ok(())
}

fn map_pairing_request(row: &rusqlite::Row<'_>) -> rusqlite::Result<PairingRequest> {
    let permissions_json: String = row.get(3)?;
    Ok(PairingRequest {
        id: row.get(0)?,
        requester_device_id: row.get(1)?,
        target_device_id: row.get(2)?,
        permissions: serde_json::from_str(&permissions_json).unwrap_or_default(),
        status: row.get(4)?,
        created_at: row.get(5)?,
        expires_at: row.get(6)?,
        resolved_at: row.get(7)?,
        resolved_by_device_id: row.get(8)?,
    })
}

fn map_device_pairing(row: &rusqlite::Row<'_>) -> rusqlite::Result<DevicePairing> {
    let permissions_json: String = row.get(3)?;
    Ok(DevicePairing {
        tenant_id: row.get(0)?,
        controller_device_id: row.get(1)?,
        target_device_id: row.get(2)?,
        permissions: serde_json::from_str(&permissions_json).unwrap_or_default(),
        created_at: row.get(4)?,
        approved_at: row.get(5)?,
    })
}

fn insert_audit(
    conn: &Connection,
    user_id: Option<&str>,
    device_id: Option<&str>,
    event_type: &str,
    metadata_json: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO auth_audit_log
         (id, user_id, device_id, event_type, metadata_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            Uuid::new_v4().to_string(),
            user_id,
            device_id,
            event_type,
            metadata_json,
            now
        ],
    )?;
    Ok(())
}

fn collect_rows<T, F>(rows: rusqlite::MappedRows<'_, F>) -> Result<Vec<T>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
