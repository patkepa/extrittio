use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::db::models::{NewFirmwareBlob, NewFirmwareUpdate};
use crate::error::AppError;
use crate::repositories::{device_type_repo, firmware_repo};
use crate::services::firmware_service;
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
    pub has_blob: bool,
    pub file_size: Option<i32>,
    pub filename: Option<String>,
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
            "/api/firmware-updates/upload",
            axum::routing::post(upload_firmware_update),
        )
        .route(
            "/api/firmware-updates/{id}",
            axum::routing::delete(delete_firmware_update),
        )
        .route(
            "/api/firmware-updates/{id}/download",
            get(download_firmware_blob),
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
) -> Result<Json<Vec<FirmwareUpdateResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    let results = firmware_repo::list_firmware_updates(&mut conn, params.device_type_id)?;

    Ok(Json(
        results
            .into_iter()
            .map(|(fw, dt, blob_size, blob_filename)| FirmwareUpdateResponse {
                id: fw.id,
                device_type_id: fw.device_type_id,
                device_type_name: dt.name,
                version: fw.version,
                url: fw.url,
                sha256: fw.sha256,
                description: fw.description,
                created_at: fw.created_at.to_string(),
                has_blob: blob_size.is_some(),
                file_size: blob_size,
                filename: blob_filename,
            })
            .collect(),
    ))
}

async fn create_firmware_update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewFirmwareUpdateRequest>,
) -> Result<(StatusCode, Json<FirmwareUpdateResponse>), AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device type exists
    let dt = device_type_repo::list_device_types(&mut conn)?
        .into_iter()
        .find(|d| d.id == body.device_type_id)
        .ok_or_else(|| AppError::NotFound(format!("Device type {} not found", body.device_type_id)))?;

    // Auto-generate version if not provided
    let version = match body.version {
        Some(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => next_version_for_type(&mut conn, body.device_type_id)?,
    };

    let new_fw = NewFirmwareUpdate {
        device_type_id: body.device_type_id,
        version: version.clone(),
        url: body.url,
        sha256: body.sha256,
        description: body.description,
    };

    let created = firmware_service::register_firmware(&mut conn, &new_fw)
        .map_err(|e| match e {
            AppError::Database(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => AppError::Conflict("Firmware version already exists for this device type".into()),
            other => other,
        })?;

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
            has_blob: false,
            file_size: None,
            filename: None,
        }),
    ))
}

#[allow(clippy::too_many_lines)]
async fn upload_firmware_update(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<FirmwareUpdateResponse>), AppError> {
    let mut device_type_id: Option<i32> = None;
    let mut version: Option<String> = None;
    let mut description: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "device_type_id" => {
                let text = field.text().await.map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?;
                device_type_id = Some(text.parse().map_err(|_| AppError::BadRequest("Invalid device_type_id".into()))?);
            }
            "version" => {
                let text = field.text().await.map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?;
                if !text.trim().is_empty() {
                    version = Some(text.trim().to_string());
                }
            }
            "description" => {
                let text = field.text().await.map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?;
                if !text.trim().is_empty() {
                    description = Some(text.trim().to_string());
                }
            }
            "file" => {
                filename = field.file_name().map(std::string::ToString::to_string);
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| AppError::BadRequest("Invalid multipart upload".into()))?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let device_type_id = device_type_id.ok_or_else(|| AppError::BadRequest("Missing device_type_id".into()))?;
    let file_data = file_data.ok_or_else(|| AppError::BadRequest("Missing file".into()))?;
    let filename = filename.unwrap_or_else(|| "firmware.bin".to_string());

    if file_data.is_empty() {
        return Err(AppError::BadRequest("File is empty".into()));
    }

    // Compute SHA-256
    let sha256_hex = Sha256::digest(&file_data)
        .iter()
        .fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        });

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let file_size = file_data.len() as i32;

    let mut conn = state.db_pool.get()?;

    // Verify device type exists
    let dt = device_type_repo::list_device_types(&mut conn)?
        .into_iter()
        .find(|d| d.id == device_type_id)
        .ok_or_else(|| AppError::NotFound(format!("Device type {device_type_id} not found")))?;

    // Auto-generate version if not provided
    let version = match version {
        Some(v) => v,
        None => next_version_for_type(&mut conn, device_type_id)?,
    };

    // Insert firmware update (with placeholder URL) and blob via service
    let new_fw = NewFirmwareUpdate {
        device_type_id,
        version: version.clone(),
        url: String::new(),
        sha256: Some(sha256_hex.clone()),
        description,
    };

    let blob = NewFirmwareBlob {
        firmware_update_id: 0, // overwritten inside upload_firmware
        data: file_data,
        size: file_size,
        filename: filename.clone(),
    };

    let updated = firmware_service::upload_firmware(&mut conn, &new_fw, blob)
        .map_err(|e| match e {
            AppError::Database(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => AppError::Conflict("Firmware version already exists for this device type".into()),
            other => other,
        })?;

    Ok((
        StatusCode::CREATED,
        Json(FirmwareUpdateResponse {
            id: updated.id,
            device_type_id: updated.device_type_id,
            device_type_name: dt.name,
            version: updated.version,
            url: updated.url,
            sha256: Some(sha256_hex),
            description: updated.description,
            created_at: updated.created_at.to_string(),
            has_blob: true,
            file_size: Some(file_size),
            filename: Some(filename),
        }),
    ))
}

async fn download_firmware_blob(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Response, AppError> {
    let mut conn = state.db_pool.get()?;

    let blob = firmware_repo::find_firmware_blob(&mut conn, id)?;

    let content_disposition = format!("attachment; filename=\"{}\"", blob.filename);

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .header(header::CONTENT_LENGTH, blob.size.to_string())
        .body(Body::from(blob.data))
        .unwrap())
}

async fn delete_firmware_update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db_pool.get()?;

    let deleted = firmware_repo::delete_firmware_update(&mut conn, id)?;

    if !deleted {
        Err(AppError::NotFound(format!("Firmware update {id} not found")))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

async fn get_next_version(
    State(state): State<Arc<AppState>>,
    Path(device_type_id): Path<i32>,
) -> Result<Json<NextVersionResponse>, AppError> {
    let mut conn = state.db_pool.get()?;

    let version = next_version_for_type(&mut conn, device_type_id)?;

    Ok(Json(NextVersionResponse {
        next_version: version,
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn next_version_for_type(
    conn: &mut diesel::SqliteConnection,
    device_type_id: i32,
) -> Result<String, diesel::result::Error> {
    let latest = firmware_repo::find_next_version(conn, device_type_id)?;

    Ok(match latest {
        Some(v) => increment_version(&v),
        None => "1.0.0".to_string(),
    })
}

fn increment_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 3
        && let Ok(patch) = parts[2].parse::<u32>() {
            return format!("{}.{}.{}", parts[0], parts[1], patch + 1);
        }
    // Fallback: append .1
    format!("{version}.1")
}
