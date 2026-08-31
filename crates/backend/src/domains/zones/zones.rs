use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use extrittio_backend_core::{CreateZone, Zone, ZoneUpdate};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateZoneRequest {
    pub name: String,
    pub description: Option<String>,
    pub geometry_type: String,
    pub geometry_json: serde_json::Value,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateZoneRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geometry_type: Option<String>,
    pub geometry_json: Option<serde_json::Value>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ZoneResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub geometry_type: String,
    pub geometry_json: serde_json::Value,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl TryFrom<Zone> for ZoneResponse {
    type Error = AppError;

    fn try_from(zone: Zone) -> Result<Self, Self::Error> {
        Ok(ZoneResponse {
            id: zone.id,
            name: zone.name,
            description: zone.description,
            geometry_type: zone.geometry_type,
            geometry_json: zone.geometry_json,
            color: zone.color,
            created_at: zone.created_at.to_rfc3339(),
            updated_at: zone.updated_at.to_rfc3339(),
        })
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/zones", get(list_zones).post(create_zone))
        .route(
            "/api/v1/zones/{zone_id}",
            get(get_zone).put(update_zone).delete(delete_zone),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/api/v1/zones", tag = "zones", security(("bearer_auth" = [])),
    responses((status = 200, body = Vec<ZoneResponse>))
)]
pub(crate) async fn list_zones(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ZoneResponse>>, AppError> {
    let zones = state
        .application()
        .zones()
        .list(&ctx.tenant_context())
        .await?;

    let responses = zones
        .into_iter()
        .map(ZoneResponse::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(responses))
}

#[utoipa::path(
    get, path = "/api/v1/zones/{zone_id}", tag = "zones", security(("bearer_auth" = [])),
    params(("zone_id" = String, Path)), responses((status = 200, body = ZoneResponse), (status = 404))
)]
pub(crate) async fn get_zone(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(zone_id): Path<String>,
) -> Result<Json<ZoneResponse>, AppError> {
    let zone = state
        .application()
        .zones()
        .get(&ctx.tenant_context(), &zone_id)
        .await?;

    Ok(Json(ZoneResponse::try_from(zone)?))
}

#[utoipa::path(
    post, path = "/api/v1/zones", tag = "zones", security(("bearer_auth" = [])),
    request_body = CreateZoneRequest, responses((status = 201, body = ZoneResponse))
)]
pub(crate) async fn create_zone(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateZoneRequest>,
) -> Result<(StatusCode, Json<ZoneResponse>), AppError> {
    let description = body.description.unwrap_or_default();
    let color = body.color.unwrap_or_else(|| "#4A90D9".to_string());

    let zone = state
        .application()
        .zones()
        .create(
            &ctx.tenant_context(),
            CreateZone {
                name: body.name,
                description,
                geometry_type: body.geometry_type,
                geometry_json: body.geometry_json,
                color,
            },
        )
        .await?;

    Ok((StatusCode::CREATED, Json(ZoneResponse::try_from(zone)?)))
}

#[utoipa::path(
    put, path = "/api/v1/zones/{zone_id}", tag = "zones", security(("bearer_auth" = [])),
    params(("zone_id" = String, Path)), request_body = UpdateZoneRequest,
    responses((status = 200, body = ZoneResponse), (status = 404))
)]
pub(crate) async fn update_zone(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(zone_id): Path<String>,
    Json(body): Json<UpdateZoneRequest>,
) -> Result<Json<ZoneResponse>, AppError> {
    let zone = state
        .application()
        .zones()
        .update(
            &ctx.tenant_context(),
            &zone_id,
            ZoneUpdate {
                name: body.name,
                description: body.description,
                geometry_type: body.geometry_type,
                geometry_json: body.geometry_json,
                color: body.color,
            },
        )
        .await?;

    Ok(Json(ZoneResponse::try_from(zone)?))
}

#[utoipa::path(
    delete, path = "/api/v1/zones/{zone_id}", tag = "zones", security(("bearer_auth" = [])),
    params(("zone_id" = String, Path)), responses((status = 204), (status = 404))
)]
pub(crate) async fn delete_zone(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Path(zone_id): Path<String>,
) -> Result<StatusCode, AppError> {
    state
        .application()
        .zones()
        .delete(&ctx.tenant_context(), &zone_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
