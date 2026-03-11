use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::db::models::{DeviceType, NewDeviceType};
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::repositories::device_type_repo;
use crate::state::{AppState, run_db};

#[derive(Debug, Serialize, ToSchema)]
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

#[derive(Debug, Deserialize, ToSchema)]
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

/// List all device types.
#[utoipa::path(
    get,
    path = "/api/v1/device-types",
    tag = "device-types",
    security(("bearer_auth" = [])),
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated list of device types", body = PaginatedResponse<DeviceTypeResponse>),
    ),
)]
pub(crate) async fn list_device_types(
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

/// Create a new device type.
#[utoipa::path(
    post,
    path = "/api/v1/device-types",
    tag = "device-types",
    security(("bearer_auth" = [])),
    request_body = NewDeviceTypeRequest,
    responses(
        (status = 201, description = "Device type created", body = DeviceTypeResponse),
        (status = 400, description = "Invalid input"),
    ),
)]
pub(crate) async fn create_device_type(
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

/// Delete a device type by ID.
#[utoipa::path(
    delete,
    path = "/api/v1/device-types/{id}",
    tag = "device-types",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Device type ID")),
    responses(
        (status = 204, description = "Device type deleted"),
        (status = 404, description = "Device type not found"),
        (status = 409, description = "Devices still reference this type"),
        (status = 422, description = "Cannot delete the default type"),
    ),
)]
pub(crate) async fn delete_device_type(
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
