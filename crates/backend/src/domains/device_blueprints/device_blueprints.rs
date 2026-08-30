use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::state::AppState;

use crate::domains::device_blueprints::blueprint_service;
use crate::domains::device_blueprints::types::{
    BlueprintDraftRecord, BlueprintRecord, BlueprintRevisionRecord,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct BlueprintDocumentRequest {
    #[schema(value_type = Object)]
    pub document: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlueprintResponse {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub latest_revision: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlueprintDraftResponse {
    pub id: String,
    pub blueprint_id: String,
    #[schema(value_type = Object)]
    pub document: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlueprintRevisionResponse {
    pub id: String,
    pub blueprint_id: String,
    pub revision: i32,
    #[schema(value_type = Object)]
    pub document: Value,
    pub document_hash: String,
    #[schema(value_type = Object)]
    pub compatibility: Value,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlueprintValidationIssueResponse {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlueprintValidationResponse {
    pub valid: bool,
    pub issues: Vec<BlueprintValidationIssueResponse>,
}

impl From<BlueprintRecord> for BlueprintResponse {
    fn from(record: BlueprintRecord) -> Self {
        Self {
            id: record.id,
            key: record.key,
            name: record.name,
            description: record.description,
            latest_revision: record.latest_revision,
            created_at: record.created_at.to_rfc3339(),
            updated_at: record.updated_at.to_rfc3339(),
        }
    }
}

impl From<BlueprintDraftRecord> for BlueprintDraftResponse {
    fn from(record: BlueprintDraftRecord) -> Self {
        Self {
            id: record.id,
            blueprint_id: record.blueprint_id,
            document: record.document,
            created_at: record.created_at.to_rfc3339(),
            updated_at: record.updated_at.to_rfc3339(),
        }
    }
}

impl From<BlueprintRevisionRecord> for BlueprintRevisionResponse {
    fn from(record: BlueprintRevisionRecord) -> Self {
        Self {
            id: record.id,
            blueprint_id: record.blueprint_id,
            revision: record.revision,
            document: record.document,
            document_hash: record.document_hash,
            compatibility: record.compatibility,
            created_at: record.created_at.to_rfc3339(),
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/device-blueprints",
            get(list_blueprints).post(create_blueprint),
        )
        .route("/api/v1/device-blueprints/{id}", get(get_blueprint))
        .route(
            "/api/v1/device-blueprints/{id}/draft",
            get(get_blueprint_draft).put(replace_blueprint_draft),
        )
        .route(
            "/api/v1/device-blueprints/{id}/draft/validate",
            post(validate_blueprint_draft),
        )
        .route(
            "/api/v1/device-blueprints/{id}/draft/publish",
            post(publish_blueprint_draft),
        )
        .route(
            "/api/v1/device-blueprints/{id}/revisions/latest",
            get(get_latest_blueprint_revision),
        )
        .route(
            "/api/v1/device-blueprint-revisions/{id}",
            get(get_blueprint_revision),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/device-blueprints",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated device blueprints", body = PaginatedResponse<BlueprintResponse>)
    )
)]
pub(crate) async fn list_blueprints(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<BlueprintResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);
    let result = blueprint_service::list(
        &ctx,
        state.persistence.device_blueprints.as_ref(),
        limit,
        offset,
    )
    .await?;
    Ok(Json(PaginatedResponse::new(
        result.records.into_iter().map(Into::into).collect(),
        result.total,
        limit,
        offset,
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/device-blueprints",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    request_body = BlueprintDocumentRequest,
    responses(
        (status = 201, description = "Blueprint and initial draft created", body = BlueprintResponse),
        (status = 400, description = "Malformed blueprint document"),
        (status = 409, description = "Blueprint key already exists")
    )
)]
pub(crate) async fn create_blueprint(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<BlueprintDocumentRequest>,
) -> Result<(StatusCode, Json<BlueprintResponse>), AppError> {
    let (blueprint, _) = blueprint_service::create(
        &ctx,
        state.persistence.device_blueprints.as_ref(),
        body.document,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(blueprint.into())))
}

#[utoipa::path(
    get,
    path = "/api/v1/device-blueprints/{id}",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Blueprint ID")),
    responses(
        (status = 200, description = "Device blueprint", body = BlueprintResponse),
        (status = 404, description = "Blueprint not found")
    )
)]
pub(crate) async fn get_blueprint(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<BlueprintResponse>, AppError> {
    Ok(Json(
        blueprint_service::get(&ctx, state.persistence.device_blueprints.as_ref(), &id)
            .await?
            .into(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/device-blueprints/{id}/draft",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Blueprint ID")),
    responses(
        (status = 200, description = "Mutable blueprint draft", body = BlueprintDraftResponse),
        (status = 404, description = "Blueprint not found")
    )
)]
pub(crate) async fn get_blueprint_draft(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<BlueprintDraftResponse>, AppError> {
    Ok(Json(
        blueprint_service::get_draft(&ctx, state.persistence.device_blueprints.as_ref(), &id)
            .await?
            .into(),
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/device-blueprints/{id}/draft",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Blueprint ID")),
    request_body = BlueprintDocumentRequest,
    responses(
        (status = 200, description = "Replaced blueprint draft", body = BlueprintDraftResponse),
        (status = 400, description = "Malformed blueprint document"),
        (status = 404, description = "Blueprint not found"),
        (status = 409, description = "Blueprint key already exists")
    )
)]
pub(crate) async fn replace_blueprint_draft(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<BlueprintDocumentRequest>,
) -> Result<Json<BlueprintDraftResponse>, AppError> {
    Ok(Json(
        blueprint_service::replace_draft(
            &ctx,
            state.persistence.device_blueprints.as_ref(),
            &id,
            body.document,
        )
        .await?
        .into(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/device-blueprints/{id}/draft/validate",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Blueprint ID")),
    responses(
        (status = 200, description = "Blueprint validation result", body = BlueprintValidationResponse),
        (status = 404, description = "Blueprint not found")
    )
)]
pub(crate) async fn validate_blueprint_draft(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<BlueprintValidationResponse>, AppError> {
    let result =
        blueprint_service::validate_draft(&ctx, state.persistence.device_blueprints.as_ref(), &id)
            .await?;
    Ok(Json(BlueprintValidationResponse {
        valid: result.valid,
        issues: result
            .issues
            .into_iter()
            .map(|issue| BlueprintValidationIssueResponse {
                path: issue.path,
                message: issue.message,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/device-blueprints/{id}/draft/publish",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Blueprint ID")),
    responses(
        (status = 201, description = "Immutable blueprint revision published", body = BlueprintRevisionResponse),
        (status = 404, description = "Blueprint not found"),
        (status = 409, description = "Draft changed during publication"),
        (status = 422, description = "Blueprint validation failed")
    )
)]
pub(crate) async fn publish_blueprint_draft(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<BlueprintRevisionResponse>), AppError> {
    let revision =
        blueprint_service::publish(&ctx, state.persistence.device_blueprints.as_ref(), &id).await?;
    Ok((StatusCode::CREATED, Json(revision.into())))
}

#[utoipa::path(
    get,
    path = "/api/v1/device-blueprints/{id}/revisions/latest",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Blueprint ID")),
    responses(
        (status = 200, description = "Latest published blueprint revision", body = BlueprintRevisionResponse),
        (status = 404, description = "No published revision")
    )
)]
pub(crate) async fn get_latest_blueprint_revision(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<BlueprintRevisionResponse>, AppError> {
    Ok(Json(
        blueprint_service::latest_revision(&ctx, state.persistence.device_blueprints.as_ref(), &id)
            .await?
            .into(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/device-blueprint-revisions/{id}",
    tag = "device-blueprints",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Blueprint revision ID")),
    responses(
        (status = 200, description = "Published blueprint revision", body = BlueprintRevisionResponse),
        (status = 404, description = "Revision not found")
    )
)]
pub(crate) async fn get_blueprint_revision(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<BlueprintRevisionResponse>, AppError> {
    Ok(Json(
        blueprint_service::get_revision(&ctx, state.persistence.device_blueprints.as_ref(), &id)
            .await?
            .into(),
    ))
}
