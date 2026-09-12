//! Relay WSS 通道。
//!
//! 建立到 Relay 的持久 WebSocket 连接，接收远程命令，转发给前端确认/执行。
//! TLS 使用与 relay_http.rs 相同的内置 CA（rustls），不依赖系统信任链。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use http::Request;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async_tls_with_config;
use tauri::Emitter;


const RELAY_CA_PEM: &[u8] = include_bytes!("../certs/localmind-relay-ca.crt");
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const RECONNECT_BASE: Duration = Duration::from_secs(2);
const MAX_RECONNECT: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayCommandEnvelope {
    pub version: String,
    pub id: String,
    #[serde(rename = "type")]
    pub envelope_type: String,
    pub from_device_id: String,
    pub to_device_id: Option<String>,
    pub tenant_id: Option<String>,
    pub command_id: Option<String>,
    pub intent_text: Option<String>,
    pub action_type: Option<String>,
    pub action_params: Option<serde_json::Value>,
    pub state: Option<String>,
    pub result_text: Option<String>,
    pub error_code: Option<String>,
    pub timestamp: i64,
}

struct WssState {
    connected: bool,
    destroyed: bool,
    reconnect_attempts: u32,
}

/// 全局 WSS 连接句柄（每个应用实例一个）。
pub struct RelayWssHandle {
    state: Arc<Mutex<WssState>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl RelayWssHandle {
    pub fn is_connected(&self) -> bool {
        // 非阻塞检查（tokio Mutex 需要 await，这里用 try_lock）
        self.state.try_lock().map(|s| s.connected).unwrap_or(false)
    }
}

fn build_tls_config() -> Result<rustls::ClientConfig, String> {
    let mut root_store = rustls::RootCertStore::empty();
    let certs = rustls_pemfile::certs(&mut std::io::BufReader::new(RELAY_CA_PEM))
        .map_err(|e| format!("failed to parse relay CA PEM: {e}"))?;
    for cert in certs {
        root_store.add(rustls::pki_types::CertificateDer::from(cert)).map_err(|e| format!("failed to add CA cert: {e}"))?;
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(config)
}

fn wss_url_from_https(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.starts_with("https://") {
        trimmed.replace("https://", "wss://") + "/ws"
    } else if trimmed.starts_with("http://") {
        trimmed.replace("http://", "ws://") + "/ws"
    } else {
        format!("wss://{trimmed}/ws")
    }
}

/// 启动 Relay WSS 后台连接。
/// 收到 command 信封时通过 Tauri 事件 `relay-command` 发给前端。
pub async fn start_relay_wss(
    app_handle: tauri::AppHandle,
    base_url: String,
    device_id: String,
    device_token: String,
) -> Result<RelayWssHandle, String> {
    let state = Arc::new(Mutex::new(WssState {
        connected: false,
        destroyed: false,
        reconnect_attempts: 0,
    }));
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let state_clone = state.clone();
    let url = wss_url_from_https(&base_url);
    let ws_endpoint = url;

    tokio::spawn(async move {
        loop {
            // 检查关闭信号
            if let Ok(()) = shutdown_rx.try_recv() {
                break;
            }
            if state_clone.lock().await.destroyed {
                break;
            }

            let tls_config = match build_tls_config() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("relay wss tls config error: {e}");
                    break;
                }
            };

            tracing::info!("relay wss connecting to {ws_endpoint}");
            let connector = tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(tls_config));
            let request = Request::builder()
                .uri(&ws_endpoint)
                .header("x-device-id", &device_id)
                .header("Authorization", format!("Bearer {}", &device_token))
                .body(())
                .unwrap();
            match connect_async_tls_with_config(request, None, false, Some(connector)).await {
                Ok((ws_stream, _)) => {
                    {
                        let mut s = state_clone.lock().await;
                        s.connected = true;
                        s.reconnect_attempts = 0;
                    }
                    let _ = app_handle.emit("relay-wss-connected", true);
                    tracing::info!("relay wss connected");

                    let (mut write, mut read) = ws_stream.split();
                    let hb_state = state_clone.clone();
                    let mut hb_interval = tokio::time::interval(HEARTBEAT_INTERVAL);

                    loop {
                        tokio::select! {
                            _ = hb_interval.tick() => {
                                let hb = RelayCommandEnvelope {
                                    version: "v1".into(),
                                    id: uuid::Uuid::new_v4().to_string(),
                                    envelope_type: "heartbeat".into(),
                                    from_device_id: device_id.clone(),
                                    to_device_id: None,
                                    tenant_id: None,
                                    command_id: None,
                                    intent_text: None,
                                    action_type: None,
                                    action_params: None,
                                    state: None,
                                    result_text: None,
                                    error_code: None,
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                };
                                if write.send(tokio_tungstenite::tungstenite::Message::Text(
                                    serde_json::to_string(&hb).unwrap_or_default()
                                )).await.is_err() {
                                    break;
                                }
                            }
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                        match serde_json::from_str::<RelayCommandEnvelope>(&text) {
                                            Ok(env) => {
                                                if env.envelope_type == "command" {
                                                    let _ = app_handle.emit("relay-command", &env);
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!("relay wss parse error: {e}");
                                            }
                                        }
                                    }
                                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => break,
                                    None => break,
                                    Some(Err(e)) => {
                                        tracing::warn!("relay wss read error: {e}");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            _ = &mut shutdown_rx => {
                                hb_state.lock().await.destroyed = true;
                                break;
                            }
                        }
                    }

                    state_clone.lock().await.connected = false;
                    let _ = app_handle.emit("relay-wss-connected", false);
                    tracing::info!("relay wss disconnected");
                }
                Err(e) => {
                    tracing::warn!("relay wss connect error: {e}");
                }
            }

            // 重连逻辑
            {
                let mut s = state_clone.lock().await;
                if s.destroyed { break; }
                s.reconnect_attempts += 1;
                if s.reconnect_attempts > MAX_RECONNECT {
                    tracing::error!("relay wss max reconnect attempts reached");
                    break;
                }
                let delay = RECONNECT_BASE * 2u32.saturating_pow(s.reconnect_attempts - 1);
                let delay = delay.min(Duration::from_secs(30));
                tracing::info!("relay wss reconnecting in {delay:?} (attempt {})", s.reconnect_attempts);
            }
            tokio::time::sleep(RECONNECT_BASE * 2u32.saturating_pow(state_clone.lock().await.reconnect_attempts - 1).min(MAX_RECONNECT)).await;
        }
    });

    Ok(RelayWssHandle {
        state,
        shutdown: Some(shutdown_tx),
    })
}

/// 启动 Relay WSS 连接（前端在 App 加载时调用）。
#[tauri::command]
pub async fn connect_relay_wss(
    app_handle: tauri::AppHandle,
    base_url: String,
    device_id: String,
    device_token: String,
) -> Result<(), String> {
    start_relay_wss(app_handle, base_url, device_id, device_token).await?;
    Ok(())
}
/// 发送状态回传到 Relay（如 running / done / failed）。
#[tauri::command]
pub async fn send_relay_state(
    base_url: String,
    device_id: String,
    device_token: String,
    to_device_id: String,
    command_id: String,
    state_value: String,
    result_text: Option<String>,
) -> Result<(), String> {
    let env = RelayCommandEnvelope {
        version: "v1".into(),
        id: uuid::Uuid::new_v4().to_string(),
        envelope_type: "state".into(),
        from_device_id: device_id,
        to_device_id: Some(to_device_id),
        tenant_id: None,
        command_id: Some(command_id),
        intent_text: None,
        action_type: None,
        action_params: None,
        state: Some(state_value),
        result_text,
        error_code: None,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    // 通过 HTTP relay_request 发送状态（复用 TLS 通道）
    let client = crate::relay_http::build_client().map_err(|e| format!("build client: {e}"))?;
    let url = format!("{}/v1/control/state", base_url.trim_end_matches('/'));
    let resp = client.post(&url)
        .header("Content-Type", "application/json")
        .header("x-device-id", &env.from_device_id)
        .header("x-device-token", &device_token)
        .json(&env)
        .send()
        .await
        .map_err(|e| format!("send state failed: {e}"))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("relay returned error: {text}"));
    }
    Ok(())
}
