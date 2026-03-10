use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::db::models::{DeviceType, FirmwareBlob, FirmwareUpdate, NewFirmwareBlob, NewFirmwareUpdate};
use crate::db::schema::{device_types, firmware_blobs, firmware_updates};
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
) -> Result<Json<Vec<FirmwareUpdateResponse>>, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut query = firmware_updates::table
        .inner_join(device_types::table)
        .left_join(firmware_blobs::table)
        .select((
            FirmwareUpdate::as_select(),
            DeviceType::as_select(),
            firmware_blobs::size.nullable::<diesel::sql_types::Nullable<diesel::sql_types::Integer>>(),
            firmware_blobs::filename.nullable::<diesel::sql_types::Nullable<diesel::sql_types::Text>>(),
        ))
        .into_boxed();

    if let Some(dt_id) = params.device_type_id {
        query = query.filter(firmware_updates::device_type_id.eq(dt_id));
    }

    let results: Vec<(FirmwareUpdate, DeviceType, Option<i32>, Option<String>)> = query
        .order(firmware_updates::created_at.desc())
        .load(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
            has_blob: false,
            file_size: None,
            filename: None,
        }),
    ))
}

async fn upload_firmware_update(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<FirmwareUpdateResponse>), StatusCode> {
    let mut device_type_id: Option<i32> = None;
    let mut version: Option<String> = None;
    let mut description: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "device_type_id" => {
                let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                device_type_id = Some(text.parse().map_err(|_| StatusCode::BAD_REQUEST)?);
            }
            "version" => {
                let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                if !text.trim().is_empty() {
                    version = Some(text.trim().to_string());
                }
            }
            "description" => {
                let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                if !text.trim().is_empty() {
                    description = Some(text.trim().to_string());
                }
            }
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| StatusCode::BAD_REQUEST)?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let device_type_id = device_type_id.ok_or(StatusCode::BAD_REQUEST)?;
    let file_data = file_data.ok_or(StatusCode::BAD_REQUEST)?;
    let filename = filename.unwrap_or_else(|| "firmware.bin".to_string());

    if file_data.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Compute SHA-256
    let sha256_hex: String = Sha256::digest(&file_data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    let file_size = file_data.len() as i32;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify device type exists
    let dt: DeviceType = device_types::table
        .find(device_type_id)
        .select(DeviceType::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    // Auto-generate version if not provided
    let version = match version {
        Some(v) => v,
        None => next_version_for_type(&mut conn, device_type_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    };

    // Insert firmware update with placeholder URL (updated after we know the ID)
    let new_fw = NewFirmwareUpdate {
        device_type_id,
        version: version.clone(),
        url: String::new(),
        sha256: Some(sha256_hex.clone()),
        description,
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

    // Get the created record
    let created: FirmwareUpdate = firmware_updates::table
        .filter(
            firmware_updates::device_type_id
                .eq(device_type_id)
                .and(firmware_updates::version.eq(&version)),
        )
        .select(FirmwareUpdate::as_select())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update URL to point to download endpoint
    let download_url = format!("/api/firmware-updates/{}/download", created.id);
    diesel::update(firmware_updates::table.find(created.id))
        .set(firmware_updates::url.eq(&download_url))
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Store the blob
    let new_blob = NewFirmwareBlob {
        firmware_update_id: created.id,
        data: file_data,
        size: file_size,
        filename: filename.clone(),
    };

    diesel::insert_into(firmware_blobs::table)
        .values(&new_blob)
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(FirmwareUpdateResponse {
            id: created.id,
            device_type_id: created.device_type_id,
            device_type_name: dt.name,
            version: created.version,
            url: download_url,
            sha256: Some(sha256_hex),
            description: created.description,
            created_at: created.created_at.to_string(),
            has_blob: true,
            file_size: Some(file_size),
            filename: Some(filename),
        }),
    ))
}

async fn download_firmware_blob(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Response, StatusCode> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let blob: FirmwareBlob = firmware_blobs::table
        .find(id)
        .select(FirmwareBlob::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

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
