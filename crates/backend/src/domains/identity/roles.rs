use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use extrittio_backend_core::{CreateRole, RoleDetails, RoleUpdate};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleResponse {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<String>,
    pub user_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionResponse {
    pub key: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRoleRequest {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<Vec<String>>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/roles", get(list_roles).post(create_role))
        .route("/api/v1/roles/permissions", get(list_permissions))
        .route(
            "/api/v1/roles/{id}",
            axum::routing::put(update_role).delete(delete_role),
        )
}

/// List roles for the current tenant.
#[utoipa::path(
    get,
    path = "/api/v1/roles",
    tag = "roles",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "Roles", body = Vec<RoleResponse>)),
)]
pub(crate) async fn list_roles(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
) -> Result<Json<Vec<RoleResponse>>, AppError> {
    let roles = state
        .application()
        .roles()
        .list(&ctx.tenant_context())
        .await?;
    Ok(Json(roles.into_iter().map(role_response).collect()))
}

/// List available permission keys.
#[utoipa::path(
    get,
    path = "/api/v1/roles/permissions",
    tag = "roles",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "Available permissions", body = Vec<PermissionResponse>)),
)]
pub(crate) async fn list_permissions(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
) -> Result<Json<Vec<PermissionResponse>>, AppError> {
    let permissions = state
        .application()
        .roles()
        .available_permissions(&ctx.tenant_context())?;
    Ok(Json(
        permissions
            .iter()
            .map(|permission| PermissionResponse {
                key: permission.key().to_string(),
            })
            .collect(),
    ))
}

/// Create a custom role.
#[utoipa::path(
    post,
    path = "/api/v1/roles",
    tag = "roles",
    security(("bearer_auth" = [])),
    request_body = CreateRoleRequest,
    responses(
        (status = 201, description = "Role created", body = RoleResponse),
        (status = 400, description = "Invalid input"),
        (status = 409, description = "Role name already exists"),
    ),
)]
pub(crate) async fn create_role(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
    Json(body): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleResponse>), AppError> {
    let role = state
        .application()
        .roles()
        .create(
            &ctx.tenant_context(),
            CreateRole {
                name: body.name,
                description: body.description,
                permissions: body.permissions,
            },
        )
        .await?;

    Ok((StatusCode::CREATED, Json(role_response(role))))
}

/// Update a custom role.
#[utoipa::path(
    put,
    path = "/api/v1/roles/{id}",
    tag = "roles",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Role ID")),
    request_body = UpdateRoleRequest,
    responses(
        (status = 200, description = "Role updated", body = RoleResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Role not found"),
        (status = 409, description = "Role name already exists"),
    ),
)]
pub(crate) async fn update_role(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
    Path(id): Path<i32>,
    Json(body): Json<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, AppError> {
    let role = state
        .application()
        .roles()
        .update(
            &ctx.tenant_context(),
            id,
            RoleUpdate {
                name: body.name,
                description: body.description.map(Some),
                permissions: body.permissions,
            },
        )
        .await?;

    Ok(Json(role_response(role)))
}

/// Delete a custom role.
#[utoipa::path(
    delete,
    path = "/api/v1/roles/{id}",
    tag = "roles",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Role ID")),
    responses(
        (status = 204, description = "Role deleted"),
        (status = 400, description = "Built-in role cannot be deleted"),
        (status = 404, description = "Role not found"),
        (status = 409, description = "Role still assigned to users"),
    ),
)]
pub(crate) async fn delete_role(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    state
        .application()
        .roles()
        .delete(&ctx.tenant_context(), id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn role_response(role: RoleDetails) -> RoleResponse {
    RoleResponse {
        id: role.role.id,
        name: role.role.name,
        description: role.role.description,
        is_system: role.role.is_system,
        permissions: role.permissions,
        user_count: role.user_count,
    }
}
