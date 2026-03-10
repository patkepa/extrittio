use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::{Fleet, NewFleet};
use crate::db::schema::{devices, fleets};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct FleetResponse {
    pub id: i32,
    pub name: String,
    pub device_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct NewFleetRequest {
    pub name: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/fleets", get(list_fleets).post(create_fleet))
        .route("/api/fleets/{id}", axum::routing::delete(delete_fleet))
}

async fn list_fleets(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<FleetResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Load all fleets, then count devices per fleet
    let all_fleets: Vec<Fleet> = fleets::table
        .select(Fleet::as_select())
        .order(fleets::name.asc())
        .load(&mut conn)?;

    let counts: Vec<(Option<i32>, i64)> = devices::table
        .group_by(devices::fleet_id)
        .select((devices::fleet_id, diesel::dsl::count(devices::id)))
        .load(&mut conn)?;

    let count_map: std::collections::HashMap<i32, i64> = counts
        .into_iter()
        .filter_map(|(fleet_id, count)| fleet_id.map(|fid| (fid, count)))
        .collect();

    let response: Vec<FleetResponse> = all_fleets
        .into_iter()
        .map(|f| FleetResponse {
            id: f.id,
            name: f.name,
            device_count: count_map.get(&f.id).copied().unwrap_or(0),
        })
        .collect();

    Ok(Json(response))
}

async fn create_fleet(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewFleetRequest>,
) -> Result<(StatusCode, Json<FleetResponse>), AppError> {
    let mut conn = state.db_pool.get()?;

    diesel::insert_into(fleets::table)
        .values(NewFleet { name: body.name })
        .execute(&mut conn)?;

    let created: Fleet = fleets::table
        .order(fleets::id.desc())
        .select(Fleet::as_select())
        .first(&mut conn)?;

    Ok((
        StatusCode::CREATED,
        Json(FleetResponse {
            id: created.id,
            name: created.name,
            device_count: 0,
        }),
    ))
}

async fn delete_fleet(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db_pool.get()?;

    // ON DELETE SET NULL in the schema handles device unassignment
    let rows = diesel::delete(fleets::table.find(id))
        .execute(&mut conn)?;

    if rows == 0 {
        Err(AppError::NotFound(format!("Fleet {id} not found")))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}
