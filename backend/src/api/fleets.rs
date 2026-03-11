use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::NewFleet;
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::repositories::fleet_repo;
use crate::state::{run_db, AppState};

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
        .route("/api/v1/fleets", get(list_fleets).post(create_fleet))
        .route("/api/v1/fleets/{id}", axum::routing::delete(delete_fleet))
}

async fn list_fleets(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<FleetResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        let (all_fleets, counts, total) = fleet_repo::list_fleets(conn, limit, offset)?;

        let count_map: std::collections::HashMap<i32, i64> = counts
            .into_iter()
            .filter_map(|(fleet_id, count)| fleet_id.map(|fid| (fid, count)))
            .collect();

        let data = all_fleets
            .into_iter()
            .map(|f| FleetResponse {
                id: f.id,
                name: f.name,
                device_count: count_map.get(&f.id).copied().unwrap_or(0),
            })
            .collect();

        Ok(PaginatedResponse::new(data, total, limit, offset))
    })
    .await?;

    Ok(Json(response))
}

async fn create_fleet(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewFleetRequest>,
) -> Result<(StatusCode, Json<FleetResponse>), AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Fleet name must not be empty".into()));
    }

    let response = run_db(&state.db_pool, move |conn| {
        let created = fleet_repo::insert_fleet(conn, &NewFleet { name: body.name })?;
        Ok(FleetResponse {
            id: created.id,
            name: created.name,
            device_count: 0,
        })
    })
    .await?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn delete_fleet(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    run_db(&state.db_pool, move |conn| {
        // ON DELETE SET NULL in the schema handles device unassignment
        let deleted = fleet_repo::delete_fleet(conn, id)?;
        if !deleted {
            Err(AppError::NotFound(format!("Fleet {id} not found")))
        } else {
            Ok(())
        }
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
