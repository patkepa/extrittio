use axum::{
    Extension, Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::context::RequestContext;
use crate::auth::create_token_with_scopes;
use crate::db::models::Role;
use crate::error::AppError;
use crate::services::user_service;
use crate::state::{AppState, run_db};
use crate::tenancy::{DEFAULT_TENANT_ID, TenantId};

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub tenant_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: i32,
    pub username: String,
    pub role: String,
    pub roles: Vec<RoleSummary>,
    pub permissions: Vec<String>,
    pub permission_version: i32,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct RoleSummary {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/me", get(me))
}

/// Authenticate and obtain a JWT token.
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
    ),
)]
pub(crate) async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let jwt_secret = state.jwt_secret.clone();
    let tenant_id = body
        .tenant_id
        .as_deref()
        .map(str::trim)
        .filter(|tenant_id| !tenant_id.is_empty())
        .unwrap_or(DEFAULT_TENANT_ID)
        .to_string();
    TenantId::new(tenant_id.clone()).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let user = run_db(&state.db_pool, move |conn| {
        user_service::authenticate(conn, &tenant_id, &body.username, &body.password)
    })
    .await?;

    let token = create_token_with_scopes(
        user.id,
        &user.username,
        &user.role,
        &user.tenant_id,
        user.permissions.clone(),
        user.permission_version,
        &jwt_secret,
    )
    .map_err(|e| AppError::Auth(e.to_string()))?;

    Ok(Json(LoginResponse {
        token,
        user: user_response(
            user.id,
            user.username,
            user.role,
            user.roles,
            user.permissions,
            user.permission_version,
        ),
    }))
}

/// Get the currently authenticated user.
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current user info", body = UserResponse),
        (status = 401, description = "Unauthorized"),
    ),
)]
pub(crate) async fn me(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<UserResponse>, AppError> {
    let user = run_db(&state.db_pool, move |conn| {
        user_service::current_user(&ctx, conn)
    })
    .await?;

    Ok(Json(user_response(
        user.id,
        user.username,
        user.role,
        user.roles,
        user.permissions,
        user.permission_version,
    )))
}

pub fn role_summary(role: Role) -> RoleSummary {
    RoleSummary {
        id: role.id,
        name: role.name,
        description: role.description,
        is_system: role.is_system,
    }
}

pub fn user_response(
    id: i32,
    username: String,
    role: String,
    roles: Vec<Role>,
    permissions: Vec<String>,
    permission_version: i32,
) -> UserResponse {
    UserResponse {
        id,
        username,
        role,
        roles: roles.into_iter().map(role_summary).collect(),
        permissions,
        permission_version,
    }
}
