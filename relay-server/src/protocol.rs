use std::{collections::HashSet, sync::OnceLock};

use localmind_shared_contract::{CommandEnvelope, EnvelopeType};
use uuid::Uuid;

use crate::error::{RelayError, Result};

const MAX_INTENT_CHARS: usize = 500;
const MAX_RESULT_CHARS: usize = 10_000;
const MAX_ACTION_PARAMS_BYTES: usize = 32 * 1024;

pub const ERROR_INVALID_ENVELOPE: &str = "100001";
pub const ERROR_UNAUTHORIZED: &str = "403002";
pub const ERROR_NOT_PAIRED: &str = "401001";
pub const ERROR_INVALID_PAIRING_CODE: &str = "401002";
pub const ERROR_PAIRING_CODE_USED: &str = "401003";
pub const ERROR_ACTION_NOT_ALLOWED: &str = "402002";
pub const ERROR_EMAIL_ALREADY_REGISTERED: &str = "403003";
pub const ERROR_INVALID_CREDENTIALS: &str = "403004";
pub const ERROR_TOKEN_INVALID: &str = "403005";
pub const ERROR_ACCOUNT_DISABLED: &str = "403006";
pub const ERROR_DEVICE_NOT_ENROLLED: &str = "403007";
pub const ERROR_PAIRING_REQUEST_EXPIRED: &str = "403008";
pub const ERROR_DEVICE_ALREADY_ENROLLED: &str = "403009";
pub const ERROR_PAIRING_REQUEST_CONFLICT: &str = "403010";

fn allowed_actions() -> &'static HashSet<&'static str> {
    static ALLOWED: OnceLock<HashSet<&'static str>> = OnceLock::new();
    ALLOWED.get_or_init(|| {
        HashSet::from([
            "device.ping",
            "device.status",
            "file.list",
            "file.read",
            "app.open",
            "doc.create",
            "file.write",
            "file.move",
            "open_app",
            "open_file",
            "read_clipboard",
            "summarize_clipboard",
            "chat_task",
        ])
    })
}

pub(crate) fn is_allowed_action(action: &str) -> bool {
    allowed_actions().contains(action)
}

pub fn validate_client_envelope(
    envelope: &CommandEnvelope,
    authenticated_device_id: &str,
    now_ms: i64,
    command_ttl_seconds: i64,
) -> Result<()> {
    if envelope.version != "v1" {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "unsupported envelope version",
        ));
    }

    if Uuid::parse_str(&envelope.id).is_err() {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "envelope id must be a UUID",
        ));
    }

    if envelope.from_device_id != authenticated_device_id {
        return Err(RelayError::forbidden(
            ERROR_UNAUTHORIZED,
            "from_device_id does not match the authenticated device",
        ));
    }

    let now = now_ms;
    let oldest = now - command_ttl_seconds.saturating_mul(1_000);
    let newest = now + 5 * 60 * 1_000;
    if envelope.timestamp < oldest || envelope.timestamp > newest {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "envelope timestamp is outside the accepted window",
        ));
    }

    if envelope.intent_text.chars().count() > MAX_INTENT_CHARS {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "intent_text is too long",
        ));
    }

    if envelope.result_text.chars().count() > MAX_RESULT_CHARS {
        return Err(RelayError::bad_request(
            ERROR_INVALID_ENVELOPE,
            "result_text is too long",
        ));
    }

    if let Some(params) = &envelope.action_params {
        let size = serde_json::to_vec(params)?.len();
        if size > MAX_ACTION_PARAMS_BYTES {
            return Err(RelayError::bad_request(
                ERROR_INVALID_ENVELOPE,
                "action_params is too large",
            ));
        }
    }

    match envelope.envelope_type {
        EnvelopeType::Command => {
            if envelope.to_device_id.is_empty() || envelope.tenant_id.is_empty() {
                return Err(RelayError::bad_request(
                    ERROR_INVALID_ENVELOPE,
                    "command requires to_device_id and tenant_id",
                ));
            }
            if Uuid::parse_str(&envelope.command_id).is_err() {
                return Err(RelayError::bad_request(
                    ERROR_INVALID_ENVELOPE,
                    "command_id must be a UUID",
                ));
            }
            if envelope.action_type.is_empty() || !is_allowed_action(&envelope.action_type) {
                return Err(RelayError::forbidden(
                    ERROR_ACTION_NOT_ALLOWED,
                    format!("action_type '{}' is not allowed", envelope.action_type),
                ));
            }
        }
        EnvelopeType::Ack | EnvelopeType::State | EnvelopeType::Error => {
            if envelope.to_device_id.is_empty() || envelope.tenant_id.is_empty() {
                return Err(RelayError::bad_request(
                    ERROR_INVALID_ENVELOPE,
                    "message requires to_device_id and tenant_id",
                ));
            }
            if Uuid::parse_str(&envelope.command_id).is_err() {
                return Err(RelayError::bad_request(
                    ERROR_INVALID_ENVELOPE,
                    "command_id must be a UUID",
                ));
            }
        }
        EnvelopeType::Heartbeat => {}
        EnvelopeType::Pair | EnvelopeType::PairConfirm => {
            return Err(RelayError::bad_request(
                ERROR_INVALID_ENVELOPE,
                "pairing is performed through the authenticated REST endpoints",
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use localmind_shared_contract::{CommandState, DeviceInfo};

    fn command() -> CommandEnvelope {
        CommandEnvelope {
            version: "v1".to_string(),
            id: Uuid::new_v4().to_string(),
            envelope_type: EnvelopeType::Command,
            from_device_id: "a".to_string(),
            to_device_id: "b".to_string(),
            tenant_id: "pair".to_string(),
            command_id: Uuid::new_v4().to_string(),
            intent_text: "ping".to_string(),
            action_type: "device.ping".to_string(),
            action_params: None,
            state: Some(CommandState::Sent),
            result_text: String::new(),
            error_code: String::new(),
            timestamp: 1_000,
            pairing_code: String::new(),
            device_info: Some(DeviceInfo {
                device_name: "n".to_string(),
                platform: "windows".to_string(),
                model: String::new(),
                os_version: String::new(),
            }),
        }
    }

    #[test]
    fn rejects_unknown_action() {
        let mut value = command();
        value.action_type = "shell.run".to_string();
        let result = validate_client_envelope(&value, "a", 1_000, 7 * 24 * 60 * 60);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), ERROR_ACTION_NOT_ALLOWED);
    }

    #[test]
    fn accepts_low_risk_command() {
        let value = command();
        assert!(validate_client_envelope(&value, "a", 1_000, 7 * 24 * 60 * 60).is_ok());
    }
}
