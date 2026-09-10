use std::{sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Path, State,
    },
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::Utc;
use futures_util::StreamExt;
use localmind_shared_contract::{CommandEnvelope, CommandState, EnvelopeType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use uuid::Uuid;

use crate::{
    config::Config,
    db::{Db, Device, Pairing},
    error::{RelayError, Result},
    hub::Hub,
    protocol::{
        validate_client_envelope, ERROR_INVALID_ENVELOPE, ERROR_NOT_PAIRED, ERROR_UNAUTHORIZED,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub hub: Hub,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: Db, config: Config) -> Self {
        Self {
            db,
            hub: Hub::default(),
            config: Arc::new(config),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    let body_limit = state.config.max_message_bytes;

    Router::new()
        .route("/health", get(health))
        .route("/v1/devices/register", post(register_device))
        .route("/v1/pairing-codes", post(issue_pairing_code))
        .route("/v1/pairings", get(list_pairings))
        .route("/v1/pairings/claim", post(claim_pairing_code))
        .route("/v1/pairings/{tenant_id}", delete(revoke_pairing))
        .route("/ws", get(websocket))
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(DefaultBodyLimit::max(body_limit))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "localmind-relay",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": "v1",
        "database": state.db.path().map(|path| path.display().to_string()),
        "now": Utc::now().timestamp_millis(),
    }))
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    device_name: String,
    platform: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    os_version: String,
}

#[derive(Debug, Serialize)]
struct RegisterResponse {
    device_id: String,
    device_token: String,
    created_at: i64,
}

async fn register_device(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>> {
    let device_name = request.device_name.trim();
    if device_name.is_empty() || device_name.chars().count() > 80 {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "device_name must contain between 1 and 80 characters",
        ));
    }
    if !matches!(request.platform.as_str(), "windows" | "android") {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "platform must be windows or android",
        ));
    }

    let (device, token) = state.db.register_device(
        device_name,
        &request.platform,
        request.model.trim(),
        request.os_version.trim(),
    )?;

    Ok(Json(RegisterResponse {
        device_id: device.id,
        device_token: token,
        created_at: device.created_at,
    }))
}

#[derive(Debug, Serialize)]
struct PairingCodeResponse {
    pairing_code: String,
    expires_at: i64,
}

async fn issue_pairing_code(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PairingCodeResponse>> {
    let device = authenticate(&state, &headers)?;
    let (pairing_code, expires_at) = state
        .db
        .issue_pairing_code(&device.id, state.config.pairing_code_ttl_seconds)?;
    Ok(Json(PairingCodeResponse {
        pairing_code,
        expires_at,
    }))
}

#[derive(Debug, Deserialize)]
struct ClaimPairingRequest {
    pairing_code: String,
}

async fn claim_pairing_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClaimPairingRequest>,
) -> Result<Json<Pairing>> {
    let device = authenticate(&state, &headers)?;
    let code = request.pairing_code.trim();
    if code.len() != 6 || !code.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(RelayError::bad_request(
            "401002",
            "pairing_code must be six digits",
        ));
    }
    let pairing = state.db.claim_pairing_code(&device.id, code)?;
    Ok(Json(pairing))
}

async fn list_pairings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Pairing>>> {
    let device = authenticate(&state, &headers)?;
    Ok(Json(state.db.list_pairings(&device.id)?))
}

async fn revoke_pairing(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<StatusCode> {
    let device = authenticate(&state, &headers)?;
    if Uuid::parse_str(&tenant_id).is_err() {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "tenant_id must be a UUID",
        ));
    }
    state.db.revoke_pairing(&tenant_id, &device.id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn websocket(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response> {
    let device = authenticate(&state, &headers)?;
    let max_message_bytes = state.config.max_message_bytes;
    Ok(ws
        .max_message_size(max_message_bytes)
        .max_frame_size(max_message_bytes)
        .on_upgrade(move |socket| websocket_session(state, socket, device))
        .into_response())
}

async fn websocket_session(state: AppState, mut socket: WebSocket, device: Device) {
    let (connection_id, mut outbound) = state.hub.register(&device.id);
    tracing::info!(device_id = %device.id, platform = %device.platform, "device connected");

    if let Err(err) = flush_pending_messages(&state, &mut socket, &device).await {
        tracing::warn!(device_id = %device.id, error = %err, "failed to flush pending messages");
    }

    let result = websocket_loop(&state, &mut socket, &mut outbound, &device).await;
    if let Err(err) = result {
        tracing::debug!(device_id = %device.id, error = %err, "websocket session ended with error");
    }

    state.hub.unregister(&device.id, connection_id);
    tracing::info!(device_id = %device.id, "device disconnected");
}

async fn flush_pending_messages(
    state: &AppState,
    socket: &mut WebSocket,
    device: &Device,
) -> Result<()> {
    let pending = state
        .db
        .pending_messages(&device.id, state.config.max_pending_messages_per_device)?;
    for message in pending {
        socket
            .send(Message::Text(message.envelope_json.into()))
            .await
            .map_err(|err| RelayError::Internal(format!("websocket send failed: {err}")))?;
        state.db.mark_message_delivered(&message.id)?;
    }
    Ok(())
}

async fn websocket_loop(
    state: &AppState,
    socket: &mut WebSocket,
    outbound: &mut broadcast::Receiver<String>,
    device: &Device,
) -> Result<()> {
    loop {
        tokio::select! {
            incoming = socket.next() => {
                let Some(incoming) = incoming else {
                    break;
                };
                let message = incoming
                    .map_err(|err| RelayError::Internal(format!("websocket receive failed: {err}")))?;
                match message {
                    Message::Text(text) => {
                        if let Err(err) = process_client_message(state, device, text.as_str()).await {
                            let error = relay_error_envelope(device, &err);
                            let payload = serde_json::to_string(&error)?;
                            socket
                                .send(Message::Text(payload.into()))
                                .await
                                .map_err(|send_err| RelayError::Internal(format!("websocket send failed: {send_err}")))?;
                        }
                    }
                    Message::Binary(_) => {
                        let err = RelayError::bad_request(ERROR_INVALID_ENVELOPE, "binary envelopes are not supported");
                        let error = relay_error_envelope(device, &err);
                        let payload = serde_json::to_string(&error)?;
                        socket.send(Message::Text(payload.into())).await
                            .map_err(|send_err| RelayError::Internal(format!("websocket send failed: {send_err}")))?;
                    }
                    Message::Ping(payload) => {
                        socket.send(Message::Pong(payload)).await
                            .map_err(|err| RelayError::Internal(format!("websocket pong failed: {err}")))?;
                    }
                    Message::Pong(_) => {}
                    Message::Close(_) => break,
                }
            }
            outbound_message = outbound.recv() => {
                match outbound_message {
                    Ok(payload) => {
                        socket
                            .send(Message::Text(payload.into()))
                            .await
                            .map_err(|err| RelayError::Internal(format!("websocket send failed: {err}")))?;
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(device_id = %device.id, skipped, "websocket outbound queue lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    Ok(())
}

async fn process_client_message(state: &AppState, device: &Device, payload: &str) -> Result<()> {
    let envelope: CommandEnvelope = serde_json::from_str(payload)?;
    validate_client_envelope(
        &envelope,
        &device.id,
        Utc::now().timestamp_millis(),
        state.config.command_ttl_seconds,
    )?;
    state.db.touch_device(&device.id)?;

    match envelope.envelope_type {
        EnvelopeType::Heartbeat => Ok(()),
        EnvelopeType::Command => process_command(state, device, envelope).await,
        EnvelopeType::State => process_state(state, device, envelope).await,
        EnvelopeType::Ack | EnvelopeType::Error => {
            ensure_pair_route(state, device, &envelope)?;
            forward_message(state, &envelope).await
        }
        EnvelopeType::Pair | EnvelopeType::PairConfirm => Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "pairing must use the REST pairing API",
        )),
    }
}

async fn process_command(
    state: &AppState,
    device: &Device,
    envelope: CommandEnvelope,
) -> Result<()> {
    ensure_pair_route(state, device, &envelope)?;
    let json = serde_json::to_string(&envelope)?;
    let record = state.db.insert_command_and_enqueue(
        &envelope,
        &json,
        state.config.max_pending_messages_per_device,
    )?;

    if record.inserted {
        try_deliver_existing(
            state,
            &envelope.to_device_id,
            &envelope.id,
            &record.envelope_json,
        )?;
    }

    let state_value = parse_command_state(&record.state).unwrap_or(CommandState::Sent);
    let ack = relay_ack(&envelope, state_value);
    let ack_json = serde_json::to_string(&ack)?;
    if !state.hub.send(&device.id, &ack_json) {
        state.db.enqueue_message(
            &ack.id,
            &device.id,
            &ack_json,
            state.config.max_pending_messages_per_device,
        )?;
    }
    Ok(())
}

async fn process_state(state: &AppState, device: &Device, envelope: CommandEnvelope) -> Result<()> {
    ensure_pair_route(state, device, &envelope)?;
    let Some(next_state) = envelope.state.clone() else {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "state envelope requires a state value",
        ));
    };

    let updated = state
        .db
        .update_command_state(&envelope.command_id, &device.id, &next_state)?;
    if updated {
        forward_message(state, &envelope).await?;
    }
    Ok(())
}

fn ensure_pair_route(state: &AppState, device: &Device, envelope: &CommandEnvelope) -> Result<()> {
    let route = state
        .db
        .route_between(&envelope.tenant_id, &device.id, &envelope.to_device_id)?;
    if route.is_none() {
        return Err(RelayError::forbidden(
            ERROR_NOT_PAIRED,
            "devices are not paired",
        ));
    }
    Ok(())
}

async fn forward_message(state: &AppState, envelope: &CommandEnvelope) -> Result<()> {
    let json = serde_json::to_string(envelope)?;
    state.db.enqueue_message(
        &envelope.id,
        &envelope.to_device_id,
        &json,
        state.config.max_pending_messages_per_device,
    )?;
    try_deliver_existing(state, &envelope.to_device_id, &envelope.id, &json)
}

fn try_deliver_existing(
    state: &AppState,
    target_device_id: &str,
    message_id: &str,
    payload: &str,
) -> Result<()> {
    if state.hub.send(target_device_id, payload) {
        state.db.mark_message_delivered(message_id)?;
    }
    Ok(())
}

fn relay_ack(original: &CommandEnvelope, state: CommandState) -> CommandEnvelope {
    let mut ack = CommandEnvelope::new_ack(original);
    ack.from_device_id = "relay".to_string();
    ack.envelope_type = EnvelopeType::Ack;
    ack.state = Some(state);
    ack
}

fn relay_error_envelope(device: &Device, error: &RelayError) -> CommandEnvelope {
    let mut envelope = CommandEnvelope::new_heartbeat(&device.id);
    envelope.envelope_type = EnvelopeType::Error;
    envelope.from_device_id = "relay".to_string();
    envelope.to_device_id = device.id.clone();
    envelope.error_code = error.code().to_string();
    envelope.result_text = error.message();
    envelope
}

fn parse_command_state(value: &str) -> Option<CommandState> {
    match value {
        "sent" => Some(CommandState::Sent),
        "delivered" => Some(CommandState::Delivered),
        "running" => Some(CommandState::Running),
        "done" => Some(CommandState::Done),
        "failed" => Some(CommandState::Failed),
        _ => None,
    }
}

fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Device> {
    let device_id = headers
        .get("x-device-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            RelayError::unauthorized(ERROR_UNAUTHORIZED, "missing x-device-id header")
        })?;

    let header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            RelayError::unauthorized(ERROR_UNAUTHORIZED, "missing authorization header")
        })?;
    let token = header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RelayError::unauthorized(ERROR_UNAUTHORIZED, "invalid bearer token"))?;

    state.db.authenticate(device_id, token)
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "code": "100001",
                "message": "route not found"
            }
        })),
    )
}
