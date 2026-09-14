use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub(crate) struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ReadyResponse {
    status: String,
    database: String,
    migrations: String,
    zenoh: String,
    workers: BTreeMap<String, String>,
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
    if !ready_token_authorized(&headers, state.runtime().health_token()) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ReadyResponse {
                status: "unauthorized".to_string(),
                database: "unknown".to_string(),
                migrations: "unknown".to_string(),
                zenoh: "unknown".to_string(),
                workers: BTreeMap::new(),
            }),
        ));
    }

    let database_ready = state
        .runtime()
        .database()
        .health()
        .await
        .is_ok_and(|health| health.reachable);
    let mut snapshot = state.runtime().readiness().snapshot();
    let snapshot_ready = state
        .rule_cache()
        .metrics()
        .is_ok_and(|metrics| metrics.ready);
    let worker_ready = snapshot
        .workers
        .get("rule-snapshots")
        .copied()
        .unwrap_or(true);
    snapshot
        .workers
        .insert("rule-snapshots".into(), worker_ready && snapshot_ready);
    let workers_ready = snapshot.workers.values().all(|ready| *ready);
    let ready =
        database_ready && snapshot.migrations_ready && snapshot.zenoh_ready && workers_ready;
    let response = ReadyResponse {
        status: if ready { "ok" } else { "unavailable" }.to_string(),
        database: if database_ready { "ok" } else { "unreachable" }.to_string(),
        migrations: if snapshot.migrations_ready {
            "ok"
        } else {
            "not-ready"
        }
        .to_string(),
        zenoh: if snapshot.zenoh_ready {
            "ok"
        } else {
            "unavailable"
        }
        .to_string(),
        workers: snapshot
            .workers
            .into_iter()
            .map(|(name, ready)| (name, if ready { "ok" } else { "unavailable" }.to_string()))
            .collect(),
    };

    if ready {
        Ok(Json(response))
    } else {
        Err((StatusCode::SERVICE_UNAVAILABLE, Json(response)))
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
