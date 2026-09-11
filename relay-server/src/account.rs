use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    account_db::{AccountDevice, AuthTokens, DevicePairing, PairingRequest, User},
    api::AppState,
    db::Device,
    error::{RelayError, Result},
    protocol::{ERROR_ACTION_NOT_ALLOWED, ERROR_TOKEN_INVALID, ERROR_UNAUTHORIZED},
};

const DEVICE_ID_HEADER: &str = "x-device-id";
const DEVICE_TOKEN_HEADER: &str = "x-device-token";
const DEFAULT_CONTROL_PERMISSIONS: &[&str] = &[
    "device.ping",
    "device.status",
    "file.list",
    "file.read",
    "app.open",
];

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub device_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: User,
    pub tokens: AuthTokens,
}

#[derive(Debug, Deserialize)]
pub struct CreatePairingRequest {
    pub target_device_id: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>> {
    let (user, tokens) = state.db.register_user(
        &request.email,
        &request.display_name,
        &request.password,
        state.config.access_token_ttl_seconds,
        state.config.refresh_token_ttl_seconds,
    )?;
    Ok(Json(AuthResponse { user, tokens }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let (user, tokens) = state.db.login_user(
        &request.email,
        &request.password,
        request.device_id.as_deref(),
        state.config.access_token_ttl_seconds,
        state.config.refresh_token_ttl_seconds,
    )?;
    Ok(Json(AuthResponse { user, tokens }))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>> {
    require_non_empty(
        &request.refresh_token,
        ERROR_TOKEN_INVALID,
        "refresh_token is required",
    )?;
    let (user, tokens) = state.db.refresh_session(
        &request.refresh_token,
        state.config.access_token_ttl_seconds,
        state.config.refresh_token_ttl_seconds,
    )?;
    Ok(Json(AuthResponse { user, tokens }))
}

pub async fn logout(
    State(state): State<AppState>,
    Json(request): Json<LogoutRequest>,
) -> Result<StatusCode> {
    require_non_empty(
        &request.refresh_token,
        ERROR_TOKEN_INVALID,
        "refresh_token is required",
    )?;
    state.db.logout_session(&request.refresh_token)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<User>> {
    Ok(Json(authenticate_account(&state, &headers)?))
}

pub async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AccountDevice>>> {
    let (user, _) = authenticate_enrolled_device(&state, &headers)?;
    Ok(Json(state.db.list_account_devices(&user.id)?))
}

pub async fn claim_device(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AccountDevice>> {
    let user = authenticate_account(&state, &headers)?;
    let device_id = required_header(&headers, DEVICE_ID_HEADER, "missing x-device-id header")?;
    let device_token = required_header(
        &headers,
        DEVICE_TOKEN_HEADER,
        "missing x-device-token header",
    )?;
    Ok(Json(state.db.enroll_device(
        &user.id,
        &device_id,
        &device_token,
    )?))
}

pub async fn remove_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_device_id): Path<String>,
) -> Result<StatusCode> {
    let (user, actor) = authenticate_enrolled_device(&state, &headers)?;
    state
        .db
        .remove_account_device(&user.id, &actor.id, &target_device_id)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_pairing_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreatePairingRequest>,
) -> Result<Json<PairingRequest>> {
    let (user, requester) = authenticate_enrolled_device(&state, &headers)?;
    let target_device_id = request.target_device_id.trim();
    if target_device_id.is_empty() {
        return Err(RelayError::bad_request(
            ERROR_UNAUTHORIZED,
            "target_device_id is required",
        ));
    }
    let permissions = normalize_permissions(&request.permissions)?;
    Ok(Json(state.db.create_pairing_request(
        &user.id,
        &requester.id,
        target_device_id,
        &permissions,
        state.config.pairing_request_ttl_seconds,
    )?))
}

pub async fn list_pairing_requests(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PairingRequest>>> {
    let (user, device) = authenticate_enrolled_device(&state, &headers)?;
    Ok(Json(state.db.list_pairing_requests(&user.id, &device.id)?))
}

pub async fn approve_pairing_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
) -> Result<Json<DevicePairing>> {
    let (user, target) = authenticate_enrolled_device(&state, &headers)?;
    Ok(Json(state.db.approve_pairing_request(
        &user.id,
        &target.id,
        &request_id,
    )?))
}

pub async fn reject_pairing_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
) -> Result<StatusCode> {
    let (user, target) = authenticate_enrolled_device(&state, &headers)?;
    state
        .db
        .reject_pairing_request(&user.id, &target.id, &request_id)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_control_pairings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DevicePairing>>> {
    let (user, device) = authenticate_enrolled_device(&state, &headers)?;
    Ok(Json(state.db.list_control_pairings(&user.id, &device.id)?))
}

pub async fn revoke_control_pairing(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<StatusCode> {
    let (user, device) = authenticate_enrolled_device(&state, &headers)?;
    state
        .db
        .revoke_control_pairing(&user.id, &device.id, &tenant_id)?;
    Ok(StatusCode::NO_CONTENT)
}

fn authenticate_account(state: &AppState, headers: &HeaderMap) -> Result<User> {
    let token = bearer_token(headers)?;
    state.db.current_user_by_access_token(token)
}

fn authenticate_enrolled_device(state: &AppState, headers: &HeaderMap) -> Result<(User, Device)> {
    let user = authenticate_account(state, headers)?;
    let device_id = required_header(headers, DEVICE_ID_HEADER, "missing x-device-id header")?;
    let device_token = required_header(
        headers,
        DEVICE_TOKEN_HEADER,
        "missing x-device-token header",
    )?;
    let device = state.db.authenticate(&device_id, &device_token)?;
    state.db.ensure_account_device(&user.id, &device.id)?;
    Ok((user, device))
}

fn bearer_token(headers: &HeaderMap) -> Result<&str> {
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            RelayError::unauthorized(ERROR_TOKEN_INVALID, "missing authorization header")
        })?;
    header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RelayError::unauthorized(ERROR_TOKEN_INVALID, "invalid bearer token"))
}

fn required_header(
    headers: &HeaderMap,
    name: &'static str,
    missing: &'static str,
) -> Result<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| RelayError::unauthorized(ERROR_UNAUTHORIZED, missing))
}

fn require_non_empty(value: &str, code: &'static str, message: &'static str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(RelayError::bad_request(code, message));
    }
    Ok(())
}

fn normalize_permissions(permissions: &[String]) -> Result<Vec<String>> {
    let defaults: Vec<String> = DEFAULT_CONTROL_PERMISSIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    let permissions = if permissions.is_empty() {
        defaults
    } else {
        permissions
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect()
    };
    if permissions.len() > 16 {
        return Err(RelayError::bad_request(
            ERROR_ACTION_NOT_ALLOWED,
            "too many permissions were requested",
        ));
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for permission in permissions {
        if !is_supported_control_permission(&permission) {
            return Err(RelayError::forbidden(
                ERROR_ACTION_NOT_ALLOWED,
                format!("permission '{permission}' is not supported"),
            ));
        }
        if seen.insert(permission.clone()) {
            normalized.push(permission);
        }
    }
    if normalized.is_empty() {
        return Err(RelayError::bad_request(
            ERROR_ACTION_NOT_ALLOWED,
            "at least one permission is required",
        ));
    }
    Ok(normalized)
}

fn is_supported_control_permission(permission: &str) -> bool {
    matches!(
        permission,
        "device.ping"
            | "device.status"
            | "file.list"
            | "file.read"
            | "app.open"
            | "doc.create"
            | "file.write"
            | "file.move"
    )
}
