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

    #[cfg(feature = "postgres")]
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[cfg(feature = "postgres")]
    #[error("Connection pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),

    #[error("Persistence error: {0}")]
    Persistence(#[from] crate::persistence::PersistenceError),

    #[error("Application error: {0}")]
    Application(#[from] extrittio_backend_core::ApplicationError),

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
            #[cfg(feature = "postgres")]
            AppError::Database(diesel::result::Error::NotFound) => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Resource not found".to_string(),
            ),
            #[cfg(feature = "postgres")]
            AppError::Database(e) => {
                tracing::error!("Database error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "Internal server error".to_string(),
                )
            }
            #[cfg(feature = "postgres")]
            AppError::Pool(e) => {
                tracing::error!("Connection pool error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "service_unavailable",
                    "Service temporarily unavailable".to_string(),
                )
            }
            AppError::Persistence(crate::persistence::PersistenceError::NotFound) => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Resource not found".to_string(),
            ),
            AppError::Persistence(
                error @ (crate::persistence::PersistenceError::UniqueViolation { .. }
                | crate::persistence::PersistenceError::ForeignKeyViolation { .. }
                | crate::persistence::PersistenceError::CheckViolation { .. }),
            ) => {
                tracing::warn!(error = %error, "Persistence constraint rejected a request");
                (
                    StatusCode::CONFLICT,
                    "conflict",
                    "The request conflicts with existing data".to_string(),
                )
            }
            AppError::Persistence(
                error @ (crate::persistence::PersistenceError::Busy { .. }
                | crate::persistence::PersistenceError::Unavailable(_)),
            ) => {
                tracing::warn!(error = %error, "Persistence is temporarily unavailable");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    "Service temporarily unavailable".to_string(),
                )
            }
            AppError::Persistence(error) => {
                tracing::error!(error = %error, "Persistence operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "Internal server error".to_string(),
                )
            }
            AppError::Application(extrittio_backend_core::ApplicationError::NotFound(message)) => {
                (StatusCode::NOT_FOUND, "not_found", message.clone())
            }
            AppError::Application(extrittio_backend_core::ApplicationError::Conflict(message)) => {
                (StatusCode::CONFLICT, "conflict", message.clone())
            }
            AppError::Application(extrittio_backend_core::ApplicationError::InvalidInput(
                message,
            )) => (StatusCode::BAD_REQUEST, "bad_request", message.clone()),
            AppError::Application(extrittio_backend_core::ApplicationError::Unauthorized) => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Unauthorized".to_string(),
            ),
            AppError::Application(extrittio_backend_core::ApplicationError::Forbidden(message)) => {
                (StatusCode::FORBIDDEN, "forbidden", message.clone())
            }
            AppError::Application(extrittio_backend_core::ApplicationError::Persistence(
                extrittio_backend_core::PersistenceError::NotFound,
            )) => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Resource not found".to_string(),
            ),
            AppError::Application(extrittio_backend_core::ApplicationError::Persistence(
                error @ (extrittio_backend_core::PersistenceError::UniqueViolation { .. }
                | extrittio_backend_core::PersistenceError::ForeignKeyViolation { .. }
                | extrittio_backend_core::PersistenceError::CheckViolation { .. }),
            )) => {
                tracing::warn!(error = %error, "Persistence constraint rejected a request");
                (
                    StatusCode::CONFLICT,
                    "conflict",
                    "The request conflicts with existing data".to_string(),
                )
            }
            AppError::Application(extrittio_backend_core::ApplicationError::Persistence(
                error @ (extrittio_backend_core::PersistenceError::Busy { .. }
                | extrittio_backend_core::PersistenceError::Unavailable(_)),
            )) => {
                tracing::warn!(error = %error, "Persistence is temporarily unavailable");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    "Service temporarily unavailable".to_string(),
                )
            }
            AppError::Application(extrittio_backend_core::ApplicationError::Persistence(error)) => {
                tracing::error!(error = %error, "Persistence operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "Internal server error".to_string(),
                )
            }
            AppError::Application(extrittio_backend_core::ApplicationError::Internal(error)) => {
                tracing::error!(%error, "Application operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Internal server error".to_string(),
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

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    use super::{AppError, scope_request_id};
    use crate::persistence::{ConstraintName, PersistenceError};

    async fn response_contract(error: AppError) -> (StatusCode, serde_json::Value) {
        scope_request_id("request-fixture".to_string(), async move {
            let response = error.into_response();
            let status = response.status();
            let body = response.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice(&body).unwrap())
        })
        .await
    }

    #[tokio::test]
    async fn public_error_contract_preserves_status_codes_and_safe_bodies() {
        let cases = [
            (
                AppError::NotFound("missing device".into()),
                StatusCode::NOT_FOUND,
                "not_found",
                "missing device",
            ),
            (
                AppError::Conflict("already exists".into()),
                StatusCode::CONFLICT,
                "conflict",
                "already exists",
            ),
            (
                AppError::BadRequest("invalid input".into()),
                StatusCode::BAD_REQUEST,
                "bad_request",
                "invalid input",
            ),
            (
                AppError::UnprocessableEntity("invalid state".into()),
                StatusCode::UNPROCESSABLE_ENTITY,
                "unprocessable_entity",
                "invalid state",
            ),
            (
                AppError::Unauthorized,
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Unauthorized",
            ),
            (
                AppError::Forbidden("missing permission".into()),
                StatusCode::FORBIDDEN,
                "forbidden",
                "missing permission",
            ),
            (
                AppError::TooManyRequests,
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests",
                "Too many requests",
            ),
            (
                AppError::Auth("secret verifier detail".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_error",
                "Authentication error",
            ),
            (
                AppError::Persistence(PersistenceError::Unavailable(
                    "private connection detail".into(),
                )),
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "Service temporarily unavailable",
            ),
            (
                AppError::Persistence(PersistenceError::UniqueViolation {
                    constraint: ConstraintName::new("private_constraint"),
                }),
                StatusCode::CONFLICT,
                "conflict",
                "The request conflicts with existing data",
            ),
            (
                AppError::Persistence(PersistenceError::Internal("private query detail".into())),
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                "Internal server error",
            ),
            (
                AppError::Zenoh("private transport detail".into()),
                StatusCode::BAD_GATEWAY,
                "device_communication_error",
                "Device communication failed",
            ),
            (
                AppError::Internal("private implementation detail".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Internal server error",
            ),
        ];

        for (error, expected_status, expected_code, expected_message) in cases {
            let (status, body) = response_contract(error).await;
            assert_eq!(status, expected_status);
            assert_eq!(body["code"], expected_code);
            assert_eq!(body["message"], expected_message);
            assert_eq!(body["error"], expected_message);
            assert_eq!(body["request_id"], "request-fixture");
            assert!(body.get("details").is_none());
        }
    }

    #[test]
    fn response_body_type_remains_sendable_through_axum() {
        fn assert_body(_: Body) {}
        assert_body(AppError::Unauthorized.into_response().into_body());
    }
}
