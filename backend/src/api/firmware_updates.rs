use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::models::{DeviceType, FirmwareUpdate, NewFirmwareUpdate};
use crate::db::schema::{device_types, firmware_updates};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct FirmwareUpdateResponse {
    pub id: i32,
    pub device_type_id: i32,
    pub device_type_name: String,
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct NewFirmwareUpdateRequest {
    pub device_type_id: i32,
    pub version: Option<String>,
    pub url: String,
    pub sha256: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListFirmwareUpdatesQuery {
    pub device_type_id: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct NextVersionResponse {
    pub next_version: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/firmware-updates",
            get(list_firmware_updates).post(create_firmware_update),
        )
        .route(
            "/api/firmware-updates/{id}",
            axum::routing::delete(delete_firmware_update),
        )
        .route(
            "/api/firmware-updates/next-version/{device_type_id}",
            get(get_next_version),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_firmware_updates(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListFirmwareUpdatesQuery>,
) -> Result<Json<Vec<FirmwareUpdateResponse>>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut query = firmware_updates::table
        .inner_join(device_types::table)
        .select((FirmwareUpdate::as_select(), DeviceType::as_select()))
        .into_boxed();

    if let Some(dt_id) = params.device_type_id {
        query = query.filter(firmware_updates::device_type_id.eq(dt_id));
    }

    let results: Vec<(FirmwareUpdate, DeviceType)> = query
        .order(firmware_updates::created_at.desc())
        .load(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        results
            .into_iter()
            .map(|(fw, dt)| FirmwareUpdateResponse {
                id: fw.id,
                device_type_id: fw.device_type_id,
                device_type_name: dt.name,
                version: fw.version,
                url: fw.url,
                sha256: fw.sha256,
                description: fw.description,
                created_at: fw.created_at.to_string(),
            })
            .collect(),
    ))
}

async fn create_firmware_update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewFirmwareUpdateRequest>,
) -> Result<(StatusCode, Json<FirmwareUpdateResponse>), StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify device type exists
    let dt: DeviceType = device_types::table
        .find(body.device_type_id)
        .select(DeviceType::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    // Auto-generate version if not provided
    let version = match body.version {
        Some(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => next_version_for_type(&mut conn, body.device_type_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    };

    let new_fw = NewFirmwareUpdate {
        device_type_id: body.device_type_id,
        version: version.clone(),
        url: body.url,
        sha256: body.sha256,
        description: body.description,
    };

    diesel::insert_into(firmware_updates::table)
        .values(&new_fw)
        .execute(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    // FIX: Query by unique constraint instead of MAX(id) to avoid race condition
    let created: FirmwareUpdate = firmware_updates::table
        .filter(
            firmware_updates::device_type_id
                .eq(body.device_type_id)
                .and(firmware_updates::version.eq(&version)),
        )
        .select(FirmwareUpdate::as_select())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(FirmwareUpdateResponse {
            id: created.id,
            device_type_id: created.device_type_id,
            device_type_name: dt.name,
            version: created.version,
            url: created.url,
            sha256: created.sha256,
            description: created.description,
            created_at: created.created_at.to_string(),
        }),
    ))
}

async fn delete_firmware_update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = diesel::delete(firmware_updates::table.find(id))
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if rows == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

async fn get_next_version(
    State(state): State<Arc<AppState>>,
    Path(device_type_id): Path<i32>,
) -> Result<Json<NextVersionResponse>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let version = next_version_for_type(&mut conn, device_type_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(NextVersionResponse {
        next_version: version,
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn next_version_for_type(
    conn: &mut SqliteConnection,
    device_type_id: i32,
) -> Result<String, diesel::result::Error> {
    // FIX: Sort by created_at DESC instead of id DESC for correct ordering
    let latest: Option<String> = firmware_updates::table
        .filter(firmware_updates::device_type_id.eq(device_type_id))
        .select(firmware_updates::version)
        .order(firmware_updates::created_at.desc())
        .first(conn)
        .optional()?;

    Ok(match latest {
        Some(v) => increment_version(&v),
        None => "1.0.0".to_string(),
    })
}

fn increment_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 3 {
        if let Ok(patch) = parts[2].parse::<u32>() {
            return format!("{}.{}.{}", parts[0], parts[1], patch + 1);
        }
    }
    // Fallback: append .1
    format!("{}.1", version)
}
