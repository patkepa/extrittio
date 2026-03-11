use axum::{
    Extension, Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::auth::{Claims, create_token, verify_password};
use crate::error::AppError;
use crate::repositories::user_repo;
use crate::state::{AppState, run_db};

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
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

    let (user_id, username, role) = run_db(&state.db_pool, move |conn| {
        let user = user_repo::find_user_by_username(conn, &body.username).map_err(|e| match e {
            diesel::result::Error::NotFound => AppError::Unauthorized,
            other => AppError::Database(other),
        })?;

        if !verify_password(&body.password, &user.password_hash) {
            return Err(AppError::Unauthorized);
        }

        Ok((user.id, user.username, user.role))
    })
    .await?;

    let token = create_token(user_id, &username, &role, &jwt_secret)
        .map_err(|e| AppError::Auth(e.to_string()))?;

    Ok(Json(LoginResponse {
        token,
        user: UserResponse {
            id: user_id,
            username,
            role,
        },
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
pub(crate) async fn me(Extension(claims): Extension<Claims>) -> Json<UserResponse> {
    Json(UserResponse {
        id: claims.sub,
        username: claims.username,
        role: claims.role,
    })
}
