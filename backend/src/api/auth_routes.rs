use axum::{
    extract::State,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::{create_token, verify_password, Claims};
use crate::error::AppError;
use crate::repositories::user_repo;
use crate::state::{run_db, AppState};

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
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/me", get(me))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let jwt_secret = state.jwt_secret.clone();

    let (user_id, username, role) =
        run_db(&state.db_pool, move |conn| {
            let user = user_repo::find_user_by_username(conn, &body.username).map_err(
                |e| match e {
                    diesel::result::Error::NotFound => AppError::Unauthorized,
                    other => AppError::Database(other),
                },
            )?;

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

async fn me(Extension(claims): Extension<Claims>) -> Json<UserResponse> {
    Json(UserResponse {
        id: claims.sub,
        username: claims.username,
        role: claims.role,
    })
}
