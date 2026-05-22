use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub(crate) struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ReadyResponse {
    status: &'static str,
    database: &'static str,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}

/// Liveness probe — always returns 200 if the process is running.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is alive", body = HealthResponse),
    ),
)]
pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Readiness probe — returns 200 only if the database is reachable.
#[utoipa::path(
    get,
    path = "/ready",
    tag = "health",
    responses(
        (status = 200, description = "Service is ready", body = ReadyResponse),
        (status = 503, description = "Service unavailable", body = ReadyResponse),
    ),
)]
pub(crate) async fn ready(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ReadyResponse>, (StatusCode, Json<ReadyResponse>)> {
    if !ready_token_authorized(&headers, state.health_token.as_deref()) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ReadyResponse {
                status: "unauthorized",
                database: "unknown",
            }),
        ));
    }

    match state.db_pool.get() {
        Ok(_) => Ok(Json(ReadyResponse {
            status: "ok",
            database: "ok",
        })),
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "unavailable",
                database: "unreachable",
            }),
        )),
    }
}

fn ready_token_authorized(headers: &HeaderMap, expected_token: Option<&str>) -> bool {
    let Some(expected_token) = expected_token else {
        return true;
    };

    header_token(headers, "x-extrittio-health-token")
        .or_else(|| bearer_token(headers))
        .is_some_and(|token| token == expected_token)
}

fn header_token<'a>(headers: &'a HeaderMap, name: &'static str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use super::ready_token_authorized;

    #[test]
    fn ready_allows_requests_when_no_token_is_configured() {
        assert!(ready_token_authorized(&HeaderMap::new(), None));
    }

    #[test]
    fn ready_accepts_health_token_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-extrittio-health-token",
            HeaderValue::from_static("ready-secret"),
        );

        assert!(ready_token_authorized(&headers, Some("ready-secret")));
    }

    #[test]
    fn ready_accepts_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer ready-secret"),
        );

        assert!(ready_token_authorized(&headers, Some("ready-secret")));
    }

    #[test]
    fn ready_rejects_missing_or_wrong_token() {
        assert!(!ready_token_authorized(
            &HeaderMap::new(),
            Some("ready-secret")
        ));

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-extrittio-health-token",
            HeaderValue::from_static("wrong-secret"),
        );

        assert!(!ready_token_authorized(&headers, Some("ready-secret")));
    }
}
