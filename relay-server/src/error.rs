use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, RelayError>;

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("{message}")]
    Api {
        status: StatusCode,
        code: &'static str,
        message: String,
    },
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

impl RelayError {
    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    pub fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::UNAUTHORIZED,
            code,
            message: message.into(),
        }
    }

    pub fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::FORBIDDEN,
            code,
            message: message.into(),
        }
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::NOT_FOUND,
            code,
            message: message.into(),
        }
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Api { code, .. } => code,
            Self::Database(_) => "100002",
            Self::Json(_) => "100001",
            Self::Internal(_) => "100002",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Api { message, .. } => message.clone(),
            Self::Database(_) | Self::Json(_) | Self::Internal(_) => self.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

impl IntoResponse for RelayError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Api { status, .. } => *status,
            Self::Database(_) | Self::Json(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let body = ErrorEnvelope {
            error: ErrorDetail {
                code: self.code(),
                message: self.message(),
            },
        };
        (status, Json(body)).into_response()
    }
}
