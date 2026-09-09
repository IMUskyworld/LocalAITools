use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// WSS 指令信封 v1 — 三端共享 Rust 类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CommandEnvelope {
    pub version: String,
    pub id: String,
    #[serde(rename = "type")]
    pub envelope_type: EnvelopeType,
    pub from_device_id: String,
    #[serde(default)]
    pub to_device_id: String,
    #[serde(default)]
    pub tenant_id: String,
    #[serde(default)]
    pub command_id: String,
    #[serde(default)]
    pub intent_text: String,
    #[serde(default)]
    pub action_type: String,
    #[serde(default)]
    pub action_params: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub state: Option<CommandState>,
    #[serde(default)]
    pub result_text: String,
    #[serde(default)]
    pub error_code: String,
    pub timestamp: i64,
    #[serde(default)]
    pub pairing_code: String,
    #[serde(default)]
    pub device_info: Option<DeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeType {
    Command,
    Ack,
    State,
    Pair,
    PairConfirm,
    Heartbeat,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CommandState {
    Sent,
    Delivered,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_name: String,
    pub platform: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub os_version: String,
}

impl CommandEnvelope {
    pub fn new_ack(original: &CommandEnvelope) -> Self {
        CommandEnvelope {
            version: "v1".to_string(),
            id: uuid::Uuid::new_v4().to_string(),
            envelope_type: EnvelopeType::Ack,
            from_device_id: original.to_device_id.clone(),
            to_device_id: original.from_device_id.clone(),
            tenant_id: original.tenant_id.clone(),
            command_id: original.command_id.clone(),
            intent_text: String::new(),
            action_type: String::new(),
            action_params: None,
            state: None,
            result_text: String::new(),
            error_code: String::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            pairing_code: String::new(),
            device_info: None,
        }
    }

    pub fn new_heartbeat(device_id: &str) -> Self {
        CommandEnvelope {
            version: "v1".to_string(),
            id: uuid::Uuid::new_v4().to_string(),
            envelope_type: EnvelopeType::Heartbeat,
            from_device_id: device_id.to_string(),
            to_device_id: String::new(),
            tenant_id: String::new(),
            command_id: String::new(),
            intent_text: String::new(),
            action_type: String::new(),
            action_params: None,
            state: None,
            result_text: String::new(),
            error_code: String::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            pairing_code: String::new(),
            device_info: None,
        }
    }
}
