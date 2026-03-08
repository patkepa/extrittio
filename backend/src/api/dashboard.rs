use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use diesel::dsl::count_star;
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;

use crate::db::schema::{devices, telemetry};
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
) -> Result<Json<DashboardStats>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_devices: i64 = devices::table
        .select(count_star())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let active_devices: i64 = devices::table
        .filter(devices::status.eq("online"))
        .select(count_star())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let offline_devices: i64 = devices::table
        .filter(devices::status.eq("offline"))
        .select(count_star())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_messages: i64 = telemetry::table
        .select(count_star())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(DashboardStats {
        total_devices,
        active_devices,
        offline_devices,
        total_messages,
    }))
}
