use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::services::user_service;
use crate::state::{AppState, run_db};

use super::auth_routes::UserResponse;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub password: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/users", get(list_users).post(create_user))
        .route("/api/v1/users/{id}", axum::routing::delete(delete_user))
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

    let response = run_db(&state.db_pool, move |conn| {
        let (results, total) = user_service::list(&ctx, conn, limit, offset)?;
        let data = results
            .into_iter()
            .map(|u| UserResponse {
                id: u.id,
                username: u.username,
                role: u.role,
            })
            .collect();
        Ok(PaginatedResponse::new(data, total, limit, offset))
    })
    .await?;

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
    let user = run_db(&state.db_pool, move |conn| {
        user_service::create(&ctx, conn, &body.username, &body.password)
    })
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(UserResponse {
            id: user.id,
            username: user.username,
            role: user.role,
        }),
    ))
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
    run_db(&state.db_pool, move |conn| {
        user_service::delete(&ctx, conn, id)
    })
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
    run_db(&state.db_pool, move |conn| {
        user_service::change_password(&ctx, conn, id, &body.password)
    })
    .await?;

    Ok(StatusCode::OK)
}
