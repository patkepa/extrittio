use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::{DeviceType, NewDeviceType};
use crate::error::AppError;
use crate::repositories::device_type_repo;
use crate::state::{run_db, AppState};

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
) -> Result<Json<Vec<DeviceTypeResponse>>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let results = device_type_repo::list_device_types(conn)?;
        Ok(results.into_iter().map(DeviceTypeResponse::from).collect())
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

        if !deleted {
            Err(AppError::NotFound(format!("Device type {id} not found")))
        } else {
            Ok(())
        }
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
