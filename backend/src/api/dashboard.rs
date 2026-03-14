use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::error::AppError;
use crate::repositories::dashboard_repo;
use crate::state::{AppState, run_db};

#[derive(Serialize, ToSchema)]
pub struct DashboardStats {
    pub total_devices: i64,
    pub active_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/dashboard/stats", get(get_stats))
}

/// Get dashboard summary statistics.
#[utoipa::path(
    get,
    path = "/api/v1/dashboard/stats",
    tag = "dashboard",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Dashboard statistics", body = DashboardStats),
    ),
)]
pub(crate) async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DashboardStats>, AppError> {
    let stats = run_db(&state.db_pool, move |conn| {
        let total_devices = dashboard_repo::get_total_devices(conn)?;
        let active_devices = dashboard_repo::get_online_devices(conn)?;
        let offline_devices = dashboard_repo::get_offline_devices(conn)?;
        let total_messages = dashboard_repo::get_total_messages(conn)?;

        Ok(DashboardStats {
            total_devices,
            active_devices,
            offline_devices,
            total_messages,
        })
    })
    .await?;

    Ok(Json(stats))
}
