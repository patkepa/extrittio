use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ReadyResponse {
    status: &'static str,
    database: &'static str,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}

/// Liveness probe — always returns 200 if the process is running.
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Readiness probe — returns 200 only if the database is reachable.
async fn ready(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ReadyResponse>, (StatusCode, Json<ReadyResponse>)> {
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
