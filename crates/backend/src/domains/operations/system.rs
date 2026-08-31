use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub(crate) struct SystemVersionResponse {
    pub version: String,
    pub commit_sha: String,
    pub build_timestamp: String,
    pub database: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/system/version", get(get_version))
}

fn env_or_unknown(key: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn runtime_version() -> String {
    std::env::var("EXTRITTIO_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// Return runtime build metadata and database readiness.
#[utoipa::path(
    get,
    path = "/api/v1/system/version",
    tag = "system",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Runtime version metadata", body = SystemVersionResponse),
        (status = 401, description = "Unauthorized"),
    ),
)]
pub(crate) async fn get_version(State(state): State<Arc<AppState>>) -> Json<SystemVersionResponse> {
    let database_status = if state
        .database
        .health()
        .await
        .is_ok_and(|health| health.reachable)
    {
        "ready"
    } else {
        "unreachable"
    };
    let database = format!(
        "{}:{database_status}",
        state.database.descriptor().kind.as_str()
    );

    Json(SystemVersionResponse {
        version: runtime_version(),
        commit_sha: env_or_unknown("EXTRITTIO_COMMIT_SHA"),
        build_timestamp: env_or_unknown("EXTRITTIO_BUILD_TIMESTAMP"),
        database,
    })
}
