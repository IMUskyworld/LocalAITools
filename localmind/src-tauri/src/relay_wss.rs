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

/// Relay WSS 信封（Windows → Relay 方向）。
///
/// 注意：这里的可选字段必须 `skip_serializing_if`，不能序列化成显式 null。
/// Relay 侧契约把这些字段声明为 String + #[serde(default)]，而 default 只在
/// 字段【缺失】时生效；显式 null 会让中继报
/// "json error: invalid type: null, expected a string" 并静默丢弃整个信封。
/// 历史症状：电脑端命令执行成功，手机端却永远收不到 running/done。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayCommandEnvelope {
    pub version: String,
    pub id: String,
    #[serde(rename = "type")]
    pub envelope_type: String,
    pub from_device_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    pub timestamp: i64,
}

struct WssState {
    connected: bool,
    destroyed: bool,
    reconnect_attempts: u32,
    /// 连接建立后可用的出站通道：send_relay_state 通过它把状态信封直接发到 WS。
    outbound: Option<tokio::sync::mpsc::UnboundedSender<String>>,
}

/// 全局 WSS 连接句柄（每个应用实例一个）。
pub struct RelayWssHandle {
    state: Arc<Mutex<WssState>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl RelayWssHandle {
    /// 通过已建立的 WebSocket 发送一个信封（状态回传走这里）。
    ///
    /// 为什么不用 HTTP：中继的路由表里【没有】 `/v1/control/state`，
    /// Windows 端却一直在 POST 它 → 404 → 手机永远收不到 running/done，
    /// 只能重试到超时（实测就是这么坏的）。WS 通道本身已有 process_state 处理逻辑。
    pub async fn send_text(&self, payload: String) -> Result<(), String> {
        let guard = self.state.lock().await;
        if !guard.connected {
            return Err("Relay 尚未连接".to_string());
        }
        match &guard.outbound {
            Some(tx) => tx.send(payload).map_err(|_| "Relay 连接已断开".to_string()),
            None => Err("Relay 连接尚未就绪".to_string()),
        }
    }

    /// 通知后台任务停止（用于「重新连接前先关掉旧连接」）。
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }

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
        outbound: None,
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
                    crate::storage::diag_log_pub(&format!("relay: TLS 配置失败 ❌ {e}"));
                    tracing::error!("relay wss tls config error: {e}");
                    break;
                }
            };

            crate::storage::diag_log_pub(&format!(
                "relay: 正在连接 {ws_endpoint}（第 {} 次尝试）",
                state_clone.lock().await.reconnect_attempts + 1
            ));
            tracing::info!("relay wss connecting to {ws_endpoint}");
            let connector = tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(tls_config));
            // 关键：走「自定义 Request」这条路时，tungstenite 不会自动补 WebSocket 握手头，
            // 少一个 Sec-WebSocket-Key 就会被中继以 "Missing ... sec-websocket-key" 拒绝
            // （实测：电脑端因此永远显示离线，而 Node/OkHttp 客户端正常）。
            let authority = http::Uri::try_from(ws_endpoint.as_str())
                .ok()
                .and_then(|u| u.authority().map(|a| a.as_str().to_string()))
                .unwrap_or_default();
            let mut ws_key_bytes = [0u8; 16];
            rand::Rng::fill(&mut rand::thread_rng(), &mut ws_key_bytes);
            let ws_key = {
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD.encode(ws_key_bytes)
            };
            let request = Request::builder()
                .uri(&ws_endpoint)
                .header("Host", authority)
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .header("Sec-WebSocket-Version", "13")
                .header("Sec-WebSocket-Key", ws_key)
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
                    crate::storage::diag_log_pub("relay: 已连接 ✅");
                    tracing::info!("relay wss connected");

                    let (mut write, mut read) = ws_stream.split();
                    let hb_state = state_clone.clone();
                    let mut hb_interval = tokio::time::interval(HEARTBEAT_INTERVAL);
                    // 出站通道：前端调用 send_relay_state 时经此写入
                    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
                    state_clone.lock().await.outbound = Some(out_tx);

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
                            Some(payload) = out_rx.recv() => {
                                if write.send(tokio_tungstenite::tungstenite::Message::Text(payload)).await.is_err() {
                                    break;
                                }
                            }
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                        match serde_json::from_str::<RelayCommandEnvelope>(&text) {
                                            Ok(env) => {
                                                if env.envelope_type == "command" {
                                                    crate::storage::diag_log_pub(&format!(
                                                        "relay: 收到远程命令 cmd={} action={} from={}",
                                                        env.command_id.clone().unwrap_or_default(),
                                                        env.action_type.clone().unwrap_or_default(),
                                                        env.from_device_id,
                                                    ));
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

                    {
                        let mut s = state_clone.lock().await;
                        s.connected = false;
                        s.outbound = None;
                    }
                    let _ = app_handle.emit("relay-wss-connected", false);
                    crate::storage::diag_log_pub("relay: 连接已断开，准备重连");
                    tracing::info!("relay wss disconnected");
                }
                Err(e) => {
                    crate::storage::diag_log_pub(&format!("relay: 连接失败 ❌ {e}"));
                    tracing::warn!("relay wss connect error: {e}");
                }
            }

            // 重连逻辑
            {
                let mut s = state_clone.lock().await;
                if s.destroyed { break; }
                s.reconnect_attempts += 1;
                // 不再"连败 N 次就永久放弃"：服务器重启、网络抖动后会一直自愈，
                // 否则用户必须重启软件才能恢复远控（实测踩过）。
                if s.reconnect_attempts == MAX_RECONNECT + 1 {
                    crate::storage::diag_log_pub(
                        "relay: 连续失败已达上限，改为持续重试（每 30 秒一次）",
                    );
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
    state: tauri::State<'_, crate::AppState>,
    base_url: String,
    device_id: String,
    device_token: String,
) -> Result<(), String> {
    // 先停掉上一条连接：Relay 的 hub 以 device_id 为键，后注册的会顶掉前一条。
    // 若同一个设备开着两条 WSS（例如重复启动/前端重复挂载），先断掉的那条会导致
    // 中继侧认为设备「离线」，而另一条其实还连着 —— 表现就是手机偶尔报「电脑端不在线」。
    {
        let mut slot = state.relay_wss.lock().await;
        if let Some(mut old) = slot.take() {
            old.shutdown();
            crate::storage::diag_log_pub("relay: 检测到重复连接请求，已先关闭上一条");
        }
    }
    let handle = start_relay_wss(app_handle, base_url, device_id, device_token).await?;
    *state.relay_wss.lock().await = Some(handle);
    Ok(())
}
/// 发送状态回传到 Relay（running / done / failed）。
///
/// 走 WebSocket：中继只在 WS 上实现了 process_state，HTTP 侧没有对应路由。
#[tauri::command]
#[allow(unused_variables)]
pub async fn send_relay_state(
    state: tauri::State<'_, crate::AppState>,
    base_url: String,
    device_id: String,
    device_token: String,
    to_device_id: String,
    // Relay 路由必需：控制配对的 tenant_id。缺了它 Relay 会判定"设备未配对"。
    tenant_id: Option<String>,
    command_id: String,
    state_value: String,
    result_text: Option<String>,
) -> Result<(), String> {
    // 先记日志再构造信封（String 会被 move 进去）
    crate::storage::diag_log_pub(&format!(
        "relay: 回传状态 {state_value} cmd={}",
        &command_id[..command_id.len().min(8)]
    ));
    let env = RelayCommandEnvelope {
        version: "v1".into(),
        id: uuid::Uuid::new_v4().to_string(),
        envelope_type: "state".into(),
        from_device_id: device_id,
        to_device_id: Some(to_device_id),
        tenant_id,
        command_id: Some(command_id),
        intent_text: None,
        action_type: None,
        action_params: None,
        state: Some(state_value),
        result_text,
        error_code: None,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };
    let payload = serde_json::to_string(&env).map_err(|e| format!("serialize state failed: {e}"))?;
    let guard = state.relay_wss.lock().await;
    match guard.as_ref() {
        Some(handle) => handle.send_text(payload).await,
        None => Err("Relay 尚未连接：状态未回传（等待重连后可重试）".to_string()),
    }
}
