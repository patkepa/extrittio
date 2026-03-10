use axum::{
    extract::State,
    routing::{get, post},
    Extension, Json, Router,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::{create_token, verify_password, Claims};
use crate::db::models::User;
use crate::db::schema::users;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i32,
    pub username: String,
    pub role: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let mut conn = state.db_pool.get()?;

    let user: User = users::table
        .filter(users::username.eq(&body.username))
        .select(User::as_select())
        .first(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => AppError::Unauthorized,
            other => AppError::Database(other),
        })?;

    if !verify_password(&body.password, &user.password_hash) {
        return Err(AppError::Unauthorized);
    }

    let token = create_token(user.id, &user.username, &user.role, &state.jwt_secret)
        .map_err(|e| AppError::Auth(e.to_string()))?;

    Ok(Json(LoginResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            role: user.role,
        },
    }))
}

async fn me(Extension(claims): Extension<Claims>) -> Json<UserResponse> {
    Json(UserResponse {
        id: claims.sub,
        username: claims.username,
        role: claims.role,
    })
}
