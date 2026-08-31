use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use extrittio_backend_core::{CreateUser, PageRequest};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::state::AppState;

use super::auth_routes::{UserResponse, user_response};

/// Plaintext credentials intentionally have no `Debug` implementation.
#[derive(Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role_ids: Option<Vec<i32>>,
}

/// Plaintext credentials intentionally have no `Debug` implementation.
#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetUserRolesRequest {
    pub role_ids: Vec<i32>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/users", get(list_users).post(create_user))
        .route("/api/v1/users/{id}", axum::routing::delete(delete_user))
        .route("/api/v1/users/{id}/roles", axum::routing::put(set_roles))
        .route(
            "/api/v1/users/{id}/password",
            axum::routing::put(change_password),
        )
}

/// List all users.
#[utoipa::path(
    get,
    path = "/api/v1/users",
    tag = "users",
    security(("bearer_auth" = [])),
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated list of users", body = PaginatedResponse<UserResponse>),
    ),
)]
pub(crate) async fn list_users(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);
    let page = PageRequest::new(limit, offset)
        .expect("HTTP pagination clamp always produces a valid page request");

    let users = state
        .application()
        .users()
        .list(&ctx.tenant_context(), page)
        .await?;
    let data = users
        .records
        .into_iter()
        .map(|details| {
            user_response(
                details.user.id,
                details.user.username,
                details.user.role,
                details.roles,
                details.permissions,
                details.user.permission_version,
            )
        })
        .collect();
    let response = PaginatedResponse::new(data, users.total, limit, offset);

    Ok(Json(response))
}

/// Create a new user.
#[utoipa::path(
    post,
    path = "/api/v1/users",
    tag = "users",
    security(("bearer_auth" = [])),
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created", body = UserResponse),
        (status = 400, description = "Invalid input"),
        (status = 409, description = "Username already exists"),
    ),
)]
pub(crate) async fn create_user(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    let details = state
        .application()
        .users()
        .create(
            &ctx.tenant_context(),
            CreateUser {
                username: body.username,
                password: body.password,
                role_ids: body.role_ids,
            },
        )
        .await?;
    let response = user_response(
        details.user.id,
        details.user.username,
        details.user.role,
        details.roles,
        details.permissions,
        details.user.permission_version,
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Delete a user by ID.
#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    tag = "users",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "User ID")),
    responses(
        (status = 204, description = "User deleted"),
        (status = 404, description = "User not found"),
    ),
)]
pub(crate) async fn delete_user(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    state
        .application()
        .users()
        .delete(&ctx.tenant_context(), id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Change a user's password.
#[utoipa::path(
    put,
    path = "/api/v1/users/{id}/password",
    tag = "users",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "User ID")),
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed"),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "User not found"),
    ),
)]
pub(crate) async fn change_password(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
    Path(id): Path<i32>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<StatusCode, AppError> {
    state
        .application()
        .users()
        .change_password(&ctx.tenant_context(), id, body.password)
        .await?;

    Ok(StatusCode::OK)
}

/// Assign roles to a user.
#[utoipa::path(
    put,
    path = "/api/v1/users/{id}/roles",
    tag = "users",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "User ID")),
    request_body = SetUserRolesRequest,
    responses(
        (status = 200, description = "Roles assigned", body = UserResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "User or role not found"),
        (status = 409, description = "Would remove the last owner"),
    ),
)]
pub(crate) async fn set_roles(
    State(state): State<Arc<AppState>>,
    axum::Extension(ctx): axum::Extension<RequestContext>,
    Path(id): Path<i32>,
    Json(body): Json<SetUserRolesRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let details = state
        .application()
        .users()
        .set_roles(&ctx.tenant_context(), id, body.role_ids)
        .await?;
    let response = user_response(
        details.user.id,
        details.user.username,
        details.user.role,
        details.roles,
        details.permissions,
        details.user.permission_version,
    );

    Ok(Json(response))
}
