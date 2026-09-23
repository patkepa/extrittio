use axum::{Extension, Json, Router, extract::State, http::StatusCode, routing::get};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::state::AppState;
use extrittio_backend_core::CreateApiKey;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub blueprint_id: Option<String>,
}

#[cfg(test)]
mod blueprint_request_tests {
    use super::*;

    #[test]
    fn api_keys_accept_blueprint_scope_and_reject_retired_scope() {
        let scoped: CreateApiKeyRequest =
            serde_json::from_value(serde_json::json!({"name":"CI","blueprint_id":"blueprint"}))
                .unwrap();
        assert_eq!(scoped.blueprint_id.as_deref(), Some("blueprint"));
        assert!(
            serde_json::from_value::<CreateApiKeyRequest>(
                serde_json::json!({"name":"CI","device_type_id":1})
            )
            .is_err()
        );
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateApiKeyResponse {
    pub id: i32,
    pub name: String,
    pub key: String,
    pub key_prefix: String,
    pub blueprint_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyResponse {
    pub id: i32,
    pub name: String,
    pub key_prefix: String,
    pub blueprint_id: Option<String>,
    pub blueprint_name: Option<String>,
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

#[utoipa::path(
    post, path = "/api/v1/api-keys", tag = "api-keys", security(("bearer_auth" = [])),
    request_body = CreateApiKeyRequest, responses((status = 201, body = CreateApiKeyResponse))
)]
pub(crate) async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), AppError> {
    let created = state
        .application()
        .api_keys()
        .create(
            &ctx.tenant_context(),
            CreateApiKey {
                name: body.name,
                blueprint_id: body.blueprint_id,
            },
        )
        .await
        .map_err(|error| match error {
            extrittio_backend_core::ApplicationError::InvalidInput(message) => {
                AppError::UnprocessableEntity(message)
            }
            other => AppError::Application(other),
        })?;
    let api_key = created.record;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id: api_key.id,
            name: api_key.name,
            key: created.plaintext,
            key_prefix: api_key.key_prefix,
            blueprint_id: api_key.blueprint_id,
        }),
    ))
}

#[utoipa::path(
    get, path = "/api/v1/api-keys", tag = "api-keys", security(("bearer_auth" = [])),
    responses((status = 200, body = Vec<ApiKeyResponse>))
)]
pub(crate) async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<Vec<ApiKeyResponse>>, AppError> {
    let keys = state
        .application()
        .api_keys()
        .list(&ctx.tenant_context())
        .await?;
    let keys = keys
        .into_iter()
        .map(|summary| ApiKeyResponse {
            id: summary.key.id,
            name: summary.key.name,
            key_prefix: summary.key.key_prefix,
            blueprint_id: summary.key.blueprint_id,
            blueprint_name: summary.blueprint_name,
            created_at: summary
                .key
                .created_at
                .naive_utc()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            last_used_at: summary.key.last_used_at.map(|timestamp| {
                timestamp
                    .naive_utc()
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string()
            }),
        })
        .collect();

    Ok(Json(keys))
}

#[utoipa::path(
    delete, path = "/api/v1/api-keys/{id}", tag = "api-keys", security(("bearer_auth" = [])),
    params(("id" = i32, Path)), responses((status = 204), (status = 404))
)]
pub(crate) async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    axum::extract::Path(id): axum::extract::Path<i32>,
) -> Result<StatusCode, AppError> {
    state
        .application()
        .api_keys()
        .delete(&ctx.tenant_context(), id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
