use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct FleetResponse {
    pub id: i32,
    pub name: String,
    pub device_count: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewFleetRequest {
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFleetRequest {
    pub name: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/fleets", get(list_fleets).post(create_fleet))
        .route(
            "/api/v1/fleets/{id}",
            axum::routing::patch(update_fleet).delete(delete_fleet),
        )
}

/// List all fleets.
#[utoipa::path(
    get,
    path = "/api/v1/fleets",
    tag = "fleets",
    security(("bearer_auth" = [])),
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated list of fleets", body = PaginatedResponse<FleetResponse>),
    ),
)]
pub(crate) async fn list_fleets(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<FleetResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);
    let (enriched, total) = state
        .application()
        .fleets()
        .list(&ctx.tenant_context(), limit, offset)
        .await?;
    let data = enriched
        .into_iter()
        .map(|fleet| FleetResponse {
            id: fleet.fleet.id,
            name: fleet.fleet.name,
            device_count: fleet.device_count,
        })
        .collect();
    let response = PaginatedResponse::new(data, total, limit, offset);

    Ok(Json(response))
}

/// Create a new fleet.
#[utoipa::path(
    post,
    path = "/api/v1/fleets",
    tag = "fleets",
    security(("bearer_auth" = [])),
    request_body = NewFleetRequest,
    responses(
        (status = 201, description = "Fleet created", body = FleetResponse),
        (status = 400, description = "Invalid input"),
    ),
)]
pub(crate) async fn create_fleet(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Json(body): Json<NewFleetRequest>,
) -> Result<(StatusCode, Json<FleetResponse>), AppError> {
    let created = state
        .application()
        .fleets()
        .create(&ctx.tenant_context(), &body.name)
        .await?;
    let response = FleetResponse {
        id: created.id,
        name: created.name,
        device_count: 0,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update a fleet (rename).
#[utoipa::path(
    patch,
    path = "/api/v1/fleets/{id}",
    tag = "fleets",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Fleet ID")),
    request_body = UpdateFleetRequest,
    responses(
        (status = 200, description = "Fleet updated", body = FleetResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Fleet not found"),
    ),
)]
pub(crate) async fn update_fleet(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<i32>,
    Json(body): Json<UpdateFleetRequest>,
) -> Result<Json<FleetResponse>, AppError> {
    let updated = state
        .application()
        .fleets()
        .rename(&ctx.tenant_context(), id, &body.name)
        .await?;
    let response = FleetResponse {
        id: updated.id,
        name: updated.name,
        device_count: 0, // caller can refetch the full list for counts
    };

    Ok(Json(response))
}

/// Delete a fleet by ID.
#[utoipa::path(
    delete,
    path = "/api/v1/fleets/{id}",
    tag = "fleets",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Fleet ID")),
    responses(
        (status = 204, description = "Fleet deleted"),
        (status = 404, description = "Fleet not found"),
    ),
)]
pub(crate) async fn delete_fleet(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    state
        .application()
        .fleets()
        .delete(&ctx.tenant_context(), id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
