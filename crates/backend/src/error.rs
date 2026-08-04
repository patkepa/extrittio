use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use std::future::Future;
use utoipa::ToSchema;

/// Standard error response body.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct ErrorBody {
    /// Stable machine-readable error category.
    pub code: &'static str,
    /// Human-readable error description.
    pub message: String,
    /// Compatibility alias for clients using the legacy response shape.
    pub error: String,
    /// Correlates the response with structured request logs.
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

tokio::task_local! {
    static REQUEST_ID: String;
}

pub async fn scope_request_id<F>(request_id: String, future: F) -> F::Output
where
    F: Future,
{
    REQUEST_ID.scope(request_id, future).await
}

#[must_use]
pub fn current_request_id() -> String {
    REQUEST_ID
        .try_with(Clone::clone)
        .unwrap_or_else(|_| "unavailable".to_string())
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
        let (status, code, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            AppError::UnprocessableEntity(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "unprocessable_entity",
                msg.clone(),
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Unauthorized".to_string(),
            ),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg.clone()),
            AppError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests",
                "Too many requests".to_string(),
            ),
            AppError::Auth(msg) => {
                tracing::error!("Auth error: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "authentication_error",
                    "Authentication error".to_string(),
                )
            }
            AppError::Database(diesel::result::Error::NotFound) => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Resource not found".to_string(),
            ),
            AppError::Database(e) => {
                tracing::error!("Database error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "Internal server error".to_string(),
                )
            }
            AppError::Pool(e) => {
                tracing::error!("Connection pool error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "service_unavailable",
                    "Service temporarily unavailable".to_string(),
                )
            }
            AppError::Serialization(e) => {
                tracing::error!("Serialization error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "serialization_error",
                    "Internal server error".to_string(),
                )
            }
            AppError::Zenoh(msg) => {
                tracing::error!("Zenoh error: {msg}");
                (
                    StatusCode::BAD_GATEWAY,
                    "device_communication_error",
                    "Device communication failed".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Internal server error".to_string(),
                )
            }
        };

        let body = ErrorBody {
            code,
            message: message.clone(),
            error: message,
            request_id: current_request_id(),
            details: None,
        };
        (status, Json(body)).into_response()
    }
}
