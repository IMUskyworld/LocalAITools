use std::{env, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub db_path: PathBuf,
    pub max_message_bytes: usize,
    pub max_pending_messages_per_device: i64,
    pub pairing_code_ttl_seconds: i64,
    pub pairing_request_ttl_seconds: i64,
    pub command_ttl_seconds: i64,
    pub access_token_ttl_seconds: i64,
    pub refresh_token_ttl_seconds: i64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let bind_addr = env::var("RELAY_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .map_err(|err| format!("invalid RELAY_BIND: {err}"))?;

        let db_path = PathBuf::from(
            env::var("RELAY_DB_PATH").unwrap_or_else(|_| "data/relay.db".to_string()),
        );

        let max_message_bytes = env::var("RELAY_MAX_MESSAGE_BYTES")
            .ok()
            .map(|value| value.parse::<usize>())
            .transpose()
            .map_err(|err| format!("invalid RELAY_MAX_MESSAGE_BYTES: {err}"))?
            .unwrap_or(64 * 1024);

        let max_pending_messages_per_device = env::var("RELAY_MAX_PENDING_MESSAGES")
            .ok()
            .map(|value| value.parse::<i64>())
            .transpose()
            .map_err(|err| format!("invalid RELAY_MAX_PENDING_MESSAGES: {err}"))?
            .unwrap_or(500);

        let pairing_code_ttl_seconds = env::var("RELAY_PAIRING_TTL_SECONDS")
            .ok()
            .map(|value| value.parse::<i64>())
            .transpose()
            .map_err(|err| format!("invalid RELAY_PAIRING_TTL_SECONDS: {err}"))?
            .unwrap_or(300);

        let pairing_request_ttl_seconds = env::var("RELAY_PAIRING_REQUEST_TTL_SECONDS")
            .ok()
            .map(|value| value.parse::<i64>())
            .transpose()
            .map_err(|err| format!("invalid RELAY_PAIRING_REQUEST_TTL_SECONDS: {err}"))?
            .unwrap_or(10 * 60);

        let command_ttl_seconds = env::var("RELAY_COMMAND_TTL_SECONDS")
            .ok()
            .map(|value| value.parse::<i64>())
            .transpose()
            .map_err(|err| format!("invalid RELAY_COMMAND_TTL_SECONDS: {err}"))?
            .unwrap_or(7 * 24 * 60 * 60);

        let access_token_ttl_seconds = env::var("RELAY_ACCESS_TOKEN_TTL_SECONDS")
            .ok()
            .map(|value| value.parse::<i64>())
            .transpose()
            .map_err(|err| format!("invalid RELAY_ACCESS_TOKEN_TTL_SECONDS: {err}"))?
            .unwrap_or(30 * 60);

        let refresh_token_ttl_seconds = env::var("RELAY_REFRESH_TOKEN_TTL_SECONDS")
            .ok()
            .map(|value| value.parse::<i64>())
            .transpose()
            .map_err(|err| format!("invalid RELAY_REFRESH_TOKEN_TTL_SECONDS: {err}"))?
            .unwrap_or(30 * 24 * 60 * 60);

        Ok(Self {
            bind_addr,
            db_path,
            max_message_bytes,
            max_pending_messages_per_device,
            pairing_code_ttl_seconds,
            pairing_request_ttl_seconds,
            command_ttl_seconds,
            access_token_ttl_seconds,
            refresh_token_ttl_seconds,
        })
    }
}
