use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use localmind_relay::{build_router, AppState, Config, Db};
use localmind_shared_contract::{CommandEnvelope, CommandState, EnvelopeType};
use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    device_id: String,
    device_token: String,
}

#[derive(Debug, Deserialize)]
struct PairingCodeResponse {
    pairing_code: String,
}

#[derive(Debug, Deserialize)]
struct ClaimResponse {
    tenant_id: String,
}

#[tokio::test]
async fn paired_devices_can_relay_state_and_replay_offline_queue() {
    let temp = tempfile::tempdir().unwrap();
    let config = Config {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        db_path: temp.path().join("relay.db"),
        max_message_bytes: 64 * 1024,
        max_pending_messages_per_device: 500,
        pairing_code_ttl_seconds: 300,
        pairing_request_ttl_seconds: 600,
        command_ttl_seconds: 7 * 24 * 60 * 60,
        access_token_ttl_seconds: 30 * 60,
        refresh_token_ttl_seconds: 30 * 24 * 60 * 60,
    };
    let db = Db::open(&config.db_path).unwrap();
    let app = build_router(AppState::new(db, config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let base_url = format!("http://{address}");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let http = reqwest::Client::new();
    let windows = register(&http, &base_url, "desktop", "windows").await;
    let android = register(&http, &base_url, "phone", "android").await;

    let pairing_code: PairingCodeResponse = http
        .post(format!("{base_url}/v1/pairing-codes"))
        .header("x-device-id", &windows.device_id)
        .header("authorization", format!("Bearer {}", windows.device_token))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let claim: ClaimResponse = http
        .post(format!("{base_url}/v1/pairings/claim"))
        .header("x-device-id", &android.device_id)
        .header("authorization", format!("Bearer {}", android.device_token))
        .json(&json!({ "pairing_code": pairing_code.pairing_code }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let (mut windows_socket, _) = connect_ws(address, &windows).await;
    let (mut android_socket, _) = connect_ws(address, &android).await;

    let first_command = command(
        &windows.device_id,
        &android.device_id,
        &claim.tenant_id,
        "device.ping",
    );
    windows_socket
        .send(Message::Text(
            serde_json::to_string(&first_command).unwrap(),
        ))
        .await
        .unwrap();

    let forwarded = receive_command(&mut android_socket, &first_command.command_id).await;
    assert_eq!(forwarded.action_type, "device.ping");
    let ack = receive_ack(&mut windows_socket, &first_command.command_id).await;
    assert_eq!(ack.state, Some(CommandState::Sent));

    let state_update = state_message(
        &android.device_id,
        &windows.device_id,
        &claim.tenant_id,
        &first_command.command_id,
    );
    android_socket
        .send(Message::Text(serde_json::to_string(&state_update).unwrap()))
        .await
        .unwrap();
    let state = receive_command(&mut windows_socket, &first_command.command_id).await;
    assert_eq!(state.envelope_type, EnvelopeType::State);
    assert_eq!(state.state, Some(CommandState::Done));

    android_socket.close(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;

    let queued_command = command(
        &windows.device_id,
        &android.device_id,
        &claim.tenant_id,
        "device.status",
    );
    windows_socket
        .send(Message::Text(
            serde_json::to_string(&queued_command).unwrap(),
        ))
        .await
        .unwrap();

    let (mut reconnected_android, _) = connect_ws(address, &android).await;
    let replayed = receive_command(&mut reconnected_android, &queued_command.command_id).await;
    assert_eq!(replayed.action_type, "device.status");

    windows_socket
        .send(Message::Text(
            serde_json::to_string(&queued_command).unwrap(),
        ))
        .await
        .unwrap();
    let duplicate =
        tokio::time::timeout(Duration::from_millis(500), reconnected_android.next()).await;
    assert!(duplicate.is_err(), "duplicate command was relayed twice");
}

async fn register(
    http: &reqwest::Client,
    base_url: &str,
    device_name: &str,
    platform: &str,
) -> RegisterResponse {
    http.post(format!("{base_url}/v1/devices/register"))
        .json(&json!({
            "device_name": device_name,
            "platform": platform,
            "model": "test",
            "os_version": "test"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn connect_ws(
    address: std::net::SocketAddr,
    device: &RegisterResponse,
) -> (
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::handshake::client::Response,
) {
    let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
    request.headers_mut().insert(
        "x-device-id",
        HeaderValue::from_str(&device.device_id).unwrap(),
    );
    request.headers_mut().insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", device.device_token)).unwrap(),
    );
    connect_async(request).await.unwrap()
}

fn command(
    from_device_id: &str,
    to_device_id: &str,
    tenant_id: &str,
    action_type: &str,
) -> CommandEnvelope {
    let mut envelope = CommandEnvelope::new_heartbeat(from_device_id);
    envelope.envelope_type = EnvelopeType::Command;
    envelope.to_device_id = to_device_id.to_string();
    envelope.tenant_id = tenant_id.to_string();
    envelope.command_id = Uuid::new_v4().to_string();
    envelope.intent_text = action_type.to_string();
    envelope.action_type = action_type.to_string();
    envelope.state = Some(CommandState::Sent);
    envelope
}

fn state_message(
    from_device_id: &str,
    to_device_id: &str,
    tenant_id: &str,
    command_id: &str,
) -> CommandEnvelope {
    let mut envelope = CommandEnvelope::new_heartbeat(from_device_id);
    envelope.envelope_type = EnvelopeType::State;
    envelope.to_device_id = to_device_id.to_string();
    envelope.tenant_id = tenant_id.to_string();
    envelope.command_id = command_id.to_string();
    envelope.state = Some(CommandState::Done);
    envelope.result_text = "ok".to_string();
    envelope
}

async fn receive_command(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    command_id: &str,
) -> CommandEnvelope {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let message = socket.next().await.unwrap().unwrap();
            if let Message::Text(text) = message {
                let envelope: CommandEnvelope = serde_json::from_str(&text).unwrap();
                if envelope.command_id == command_id {
                    return envelope;
                }
            }
        }
    })
    .await
    .expect("timed out waiting for command")
}

async fn receive_ack(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    command_id: &str,
) -> CommandEnvelope {
    receive_command(socket, command_id).await
}
