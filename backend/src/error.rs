use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use utoipa::ToSchema;

/// Standard error response body.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Too many requests")]
    TooManyRequests,

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Connection pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Zenoh error: {0}")]
    Zenoh(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::UnprocessableEntity(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "Too many requests".to_string()),
            AppError::Auth(msg) => {
                tracing::error!("Auth error: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Authentication error".to_string(),
                )
            }
            AppError::Database(diesel::result::Error::NotFound) => {
                (StatusCode::NOT_FOUND, "Resource not found".to_string())
            }
            AppError::Database(e) => {
                tracing::error!("Database error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Pool(e) => {
                tracing::error!("Connection pool error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Service temporarily unavailable".to_string(),
                )
            }
            AppError::Serialization(e) => {
                tracing::error!("Serialization error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Zenoh(msg) => {
                tracing::error!("Zenoh error: {msg}");
                (
                    StatusCode::BAD_GATEWAY,
                    "Device communication failed".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        let body = json!({ "error": message });
        (status, Json(body)).into_response()
    }
}
