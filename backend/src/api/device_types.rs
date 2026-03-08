use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::{DeviceType, NewDeviceType};
use crate::db::schema::{device_types, devices};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct DeviceTypeResponse {
    pub id: i32,
    pub name: String,
}

impl From<DeviceType> for DeviceTypeResponse {
    fn from(dt: DeviceType) -> Self {
        Self {
            id: dt.id,
            name: dt.name,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct NewDeviceTypeRequest {
    pub name: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/device-types",
            get(list_device_types).post(create_device_type),
        )
        .route(
            "/api/device-types/{id}",
            axum::routing::delete(delete_device_type),
        )
}

async fn list_device_types(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DeviceTypeResponse>>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<DeviceType> = device_types::table
        .select(DeviceType::as_select())
        .order(device_types::name.asc())
        .load(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        results.into_iter().map(DeviceTypeResponse::from).collect(),
    ))
}

async fn create_device_type(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewDeviceTypeRequest>,
) -> Result<(StatusCode, Json<DeviceTypeResponse>), StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    diesel::insert_into(device_types::table)
        .values(NewDeviceType { name: body.name })
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let created: DeviceType = device_types::table
        .order(device_types::id.desc())
        .select(DeviceType::as_select())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(DeviceTypeResponse::from(created))))
}

async fn delete_device_type(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prevent deleting the "default" device type (id=1)
    if id == 1 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    // Reject if any devices still reference this type
    let count: i64 = devices::table
        .filter(devices::device_type_id.eq(id))
        .count()
        .get_result(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if count > 0 {
        return Err(StatusCode::CONFLICT);
    }

    let rows = diesel::delete(device_types::table.find(id))
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if rows == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}
