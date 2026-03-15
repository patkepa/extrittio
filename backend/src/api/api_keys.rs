use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::api_key_util;
use crate::auth::Claims;
use crate::error::AppError;
use crate::services::api_key_service;
use crate::state::{run_db, AppState};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub device_type_id: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateApiKeyResponse {
    pub id: i32,
    pub name: String,
    pub key: String,
    pub key_prefix: String,
    pub device_type_id: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyResponse {
    pub id: i32,
    pub name: String,
    pub key_prefix: String,
    pub device_type_id: Option<i32>,
    pub device_type_name: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/api-keys", get(list_api_keys).post(create_api_key))
        .route(
            "/api/v1/api-keys/{id}",
            axum::routing::delete(delete_api_key),
        )
}

fn require_admin(claims: &Claims) -> Result<(), AppError> {
    if claims.role != "admin" {
        return Err(AppError::Forbidden("Admin role required".into()));
    }
    Ok(())
}

async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), AppError> {
    require_admin(&claims)?;

    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("name is required".into()));
    }

    let plaintext_key = api_key_util::generate_api_key();
    let key_hash = api_key_util::hash_api_key(&plaintext_key);
    let key_prefix = api_key_util::key_prefix(&plaintext_key);

    let new_key = crate::db::models::NewApiKey {
        name: body.name.clone(),
        key_hash,
        key_prefix: key_prefix.clone(),
        device_type_id: body.device_type_id,
    };

    let api_key = run_db(&state.db_pool, move |conn| {
        api_key_service::create(conn, &new_key)
    })
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id: api_key.id,
            name: api_key.name,
            key: plaintext_key,
            key_prefix,
            device_type_id: api_key.device_type_id,
        }),
    ))
}

async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<ApiKeyResponse>>, AppError> {
    require_admin(&claims)?;

    let keys = run_db(&state.db_pool, move |conn| {
        let keys_with_names = api_key_service::list_with_type_names(conn)?;

        let responses: Vec<ApiKeyResponse> = keys_with_names
            .into_iter()
            .map(|(k, dt_name)| ApiKeyResponse {
                id: k.id,
                name: k.name,
                key_prefix: k.key_prefix,
                device_type_id: k.device_type_id,
                device_type_name: dt_name,
                created_at: k.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
                last_used_at: k
                    .last_used_at
                    .map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
            })
            .collect();
        Ok(responses)
    })
    .await?;

    Ok(Json(keys))
}

async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<StatusCode, AppError> {
    require_admin(&claims)?;

    run_db(&state.db_pool, move |conn| {
        api_key_service::delete(conn, id)
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
