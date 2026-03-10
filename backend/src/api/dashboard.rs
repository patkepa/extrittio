use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::error::AppError;
use crate::repositories::dashboard_repo;
use crate::state::AppState;

#[derive(Serialize)]
pub struct DashboardStats {
    pub total_devices: i64,
    pub active_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/dashboard/stats", get(get_stats))
}

async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DashboardStats>, AppError> {
    let mut conn = state.db_pool.get()?;

    let total_devices = dashboard_repo::get_total_devices(&mut conn)?;
    let active_devices = dashboard_repo::get_online_devices(&mut conn)?;
    let offline_devices = dashboard_repo::get_offline_devices(&mut conn)?;
    let total_messages = dashboard_repo::get_total_firmware(&mut conn)?;

    Ok(Json(DashboardStats {
        total_devices,
        active_devices,
        offline_devices,
        total_messages,
    }))
}
