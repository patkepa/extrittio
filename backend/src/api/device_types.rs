use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::{DeviceType, NewDeviceType};
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::repositories::device_type_repo;
use crate::state::{AppState, run_db};

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
            "/api/v1/device-types",
            get(list_device_types).post(create_device_type),
        )
        .route(
            "/api/v1/device-types/{id}",
            axum::routing::delete(delete_device_type),
        )
}

async fn list_device_types(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<DeviceTypeResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        let (results, total) = device_type_repo::list_device_types(conn, limit, offset)?;
        let data = results.into_iter().map(DeviceTypeResponse::from).collect();
        Ok(PaginatedResponse::new(data, total, limit, offset))
    })
    .await?;

    Ok(Json(response))
}

async fn create_device_type(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewDeviceTypeRequest>,
) -> Result<(StatusCode, Json<DeviceTypeResponse>), AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Device type name must not be empty".into(),
        ));
    }

    let response = run_db(&state.db_pool, move |conn| {
        let created =
            device_type_repo::insert_device_type(conn, &NewDeviceType { name: body.name })?;
        Ok(DeviceTypeResponse::from(created))
    })
    .await?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn delete_device_type(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    run_db(&state.db_pool, move |conn| {
        // Prevent deleting the "default" device type (id=1)
        if id == 1 {
            return Err(AppError::UnprocessableEntity(
                "Cannot delete the default device type".into(),
            ));
        }

        // Reject if any devices still reference this type
        let count = device_type_repo::count_devices_for_type(conn, id)?;

        if count > 0 {
            return Err(AppError::Conflict(format!(
                "Cannot delete device type: {count} device(s) still reference it"
            )));
        }

        let deleted = device_type_repo::delete_device_type(conn, id)?;

        if deleted {
            Ok(())
        } else {
            Err(AppError::NotFound(format!("Device type {id} not found")))
        }
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
