use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::api_key_util;
use crate::db::models::NewFirmwareUpdate;
use crate::error::AppError;
use crate::repositories::{api_key_repo, device_type_repo, firmware_repo};
use crate::state::{run_db, AppState};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CiIngestRequest {
    pub device_type: String,
    pub version: String,
    pub artifact_url: String,
    pub sha256: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<String>,
    pub description: Option<String>,
    pub changelog: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CiIngestResponse {
    pub id: i32,
    pub version: String,
    pub device_type: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/firmware-updates/ci", post(ci_ingest))
}

fn extract_api_key(headers: &HeaderMap) -> Result<String, AppError> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let key = auth
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    if !key.starts_with("extr_") {
        return Err(AppError::Unauthorized);
    }

    Ok(key.to_string())
}

async fn ci_ingest(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CiIngestRequest>,
) -> Result<(StatusCode, Json<CiIngestResponse>), AppError> {
    // 1. Validate API key
    let plaintext_key = extract_api_key(&headers)?;
    let key_hash = api_key_util::hash_api_key(&plaintext_key);

    // 2. Rate limit by API key
    if !state.ci_rate_limiter.check(&key_hash) {
        return Err(AppError::TooManyRequests);
    }

    // 3. Validate required fields
    if body.artifact_url.is_empty()
        || (!body.artifact_url.starts_with("http://")
            && !body.artifact_url.starts_with("https://"))
    {
        return Err(AppError::UnprocessableEntity(
            "artifact_url must be a valid http:// or https:// URL".into(),
        ));
    }

    // 3. Parse build_timestamp if provided
    let build_ts = body
        .build_timestamp
        .as_deref()
        .map(|ts| {
            NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ"))
                .or_else(|_| {
                    chrono::DateTime::parse_from_rfc3339(ts).map(|dt| dt.naive_utc())
                })
                .map_err(|_| {
                    AppError::UnprocessableEntity(
                        "build_timestamp must be ISO 8601 format".into(),
                    )
                })
        })
        .transpose()?;

    let device_type_name = body.device_type.clone();
    let version = body.version.clone();

    let result = run_db(&state.db_pool, move |conn| {
        // 4. Look up API key
        let api_key = api_key_repo::find_api_key_by_hash(conn, &key_hash)?
            .ok_or(AppError::Unauthorized)?;

        // 5. Update last_used_at
        let _ = api_key_repo::update_last_used(conn, api_key.id);

        // 6. Resolve device type by name
        let device_type = device_type_repo::find_device_type_by_name(conn, &device_type_name)?
            .ok_or_else(|| {
                AppError::NotFound(format!("Device type '{}' not found", device_type_name))
            })?;

        // 7. Check scope
        if let Some(scoped_id) = api_key.device_type_id {
            if scoped_id != device_type.id {
                return Err(AppError::Forbidden(format!(
                    "API key is scoped to device type ID {}, not '{}'",
                    scoped_id, device_type_name
                )));
            }
        }

        // 8. Insert firmware update
        let new_fw = NewFirmwareUpdate {
            device_type_id: device_type.id,
            version: version.clone(),
            url: body.artifact_url.clone(),
            description: body.description.clone(),
            sha256: body.sha256.clone(),
            commit_sha: body.commit_sha.clone(),
            branch: body.branch.clone(),
            ci_run_url: body.ci_run_url.clone(),
            build_timestamp: build_ts,
            changelog: body.changelog.clone(),
            source: Some("ci".to_string()),
        };

        let fw = firmware_repo::insert_firmware_update(conn, &new_fw).map_err(|e| {
            if let diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ) = &e
            {
                AppError::Conflict(format!(
                    "Version '{}' already exists for device type '{}'",
                    version, device_type_name
                ))
            } else {
                AppError::Internal(e.to_string())
            }
        })?;

        Ok((fw.id, fw.version, device_type_name))
    })
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CiIngestResponse {
            id: result.0,
            version: result.1,
            device_type: result.2,
        }),
    ))
}
