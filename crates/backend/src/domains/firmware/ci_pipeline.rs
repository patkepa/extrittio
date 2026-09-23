use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::api_key_util;
use crate::error::AppError;
use crate::security;
use crate::state::AppState;
use extrittio_backend_core::CiIngestParams;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CiIngestRequest {
    pub blueprint_revision_id: String,
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

#[cfg(test)]
mod blueprint_request_tests {
    use super::*;

    #[test]
    fn ci_requires_revision_and_rejects_device_type() {
        let mut input = serde_json::json!({
            "blueprint_revision_id":"revision", "version":"1",
            "artifact_url":"https://example.test/fw.bin"
        });
        assert!(serde_json::from_value::<CiIngestRequest>(input.clone()).is_ok());
        input["device_type"] = "sensor".into();
        assert!(serde_json::from_value::<CiIngestRequest>(input.clone()).is_err());
        input
            .as_object_mut()
            .unwrap()
            .remove("blueprint_revision_id");
        assert!(serde_json::from_value::<CiIngestRequest>(input).is_err());
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CiIngestResponse {
    pub id: i32,
    pub version: String,
    pub blueprint_revision_id: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/firmware-updates/ci", post(ci_ingest))
}

fn extract_api_key(headers: &HeaderMap) -> Result<String, AppError> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let key = auth.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;

    if !key.starts_with("extr_") {
        return Err(AppError::Unauthorized);
    }

    Ok(key.to_string())
}

#[utoipa::path(
    post, path = "/api/v1/firmware-updates/ci", tag = "firmware",
    request_body = CiIngestRequest,
    responses((status = 201, body = CiIngestResponse), (status = 401), (status = 422))
)]
pub(crate) async fn ci_ingest(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CiIngestRequest>,
) -> Result<(StatusCode, Json<CiIngestResponse>), AppError> {
    // 1. Validate API key
    let plaintext_key = extract_api_key(&headers)?;
    let key_hash = api_key_util::hash_api_key(&plaintext_key);

    // 2. Rate limit by API key
    if !state.http().ci_rate_limiter().check(&key_hash) {
        return Err(AppError::TooManyRequests);
    }

    // 3. Validate required fields
    if body.artifact_url.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "artifact_url must be a valid HTTPS URL".into(),
        ));
    }
    security::validate_public_https_url(&body.artifact_url, "artifact_url")
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;
    if !body
        .sha256
        .as_deref()
        .is_some_and(|hash| hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(AppError::UnprocessableEntity(
            "sha256 is required and must be a 64-character hex digest".into(),
        ));
    }

    // 3. Parse build_timestamp if provided
    let build_ts = body
        .build_timestamp
        .as_deref()
        .map(|ts| {
            NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ"))
                .or_else(|_| chrono::DateTime::parse_from_rfc3339(ts).map(|dt| dt.naive_utc()))
                .map_err(|_| {
                    AppError::UnprocessableEntity("build_timestamp must be ISO 8601 format".into())
                })
        })
        .transpose()?;

    let result = state
        .application()
        .ci_ingest()
        .ingest(
            &key_hash,
            CiIngestParams {
                blueprint_revision_id: body.blueprint_revision_id,
                version: body.version,
                artifact_url: body.artifact_url,
                sha256: body.sha256,
                commit_sha: body.commit_sha,
                branch: body.branch,
                ci_run_url: body.ci_run_url,
                build_timestamp: build_ts.map(|value| value.and_utc()),
                description: body.description,
                changelog: body.changelog,
            },
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CiIngestResponse {
            id: result.0,
            version: result.1,
            blueprint_revision_id: result.2,
        }),
    ))
}
