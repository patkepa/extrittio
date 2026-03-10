use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::hash_password;
use crate::db::models::NewUser;
use crate::error::AppError;
use crate::repositories::user_repo;
use crate::state::AppState;

use super::auth_routes::UserResponse;

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub password: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/users", get(list_users).post(create_user))
        .route(
            "/api/v1/users/{id}",
            axum::routing::delete(delete_user),
        )
        .route(
            "/api/v1/users/{id}/password",
            axum::routing::put(change_password),
        )
}

async fn list_users(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    let results = user_repo::list_users(&mut conn)?;

    let response: Vec<UserResponse> = results
        .into_iter()
        .map(|u| UserResponse {
            id: u.id,
            username: u.username,
            role: u.role,
        })
        .collect();

    Ok(Json(response))
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    let mut conn = state.db_pool.get()?;

    let password_hash = hash_password(&body.password)
        .map_err(|e| AppError::Auth(e.to_string()))?;

    let new_user = NewUser {
        username: body.username.clone(),
        password_hash,
    };

    let user = user_repo::insert_user(&mut conn, &new_user)
        .map_err(|e| match e {
            diesel::result::Error::DatabaseError(diesel::result::DatabaseErrorKind::UniqueViolation, _) => {
                AppError::Conflict(format!("Username '{}' already exists", body.username))
            }
            other => AppError::Database(other),
        })?;

    Ok((
        StatusCode::CREATED,
        Json(UserResponse {
            id: user.id,
            username: user.username,
            role: user.role,
        }),
    ))
}

async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db_pool.get()?;

    let deleted = user_repo::delete_user(&mut conn, id)?;

    if !deleted {
        return Err(AppError::NotFound(format!("User {id} not found")));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn change_password(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify user exists
    user_repo::find_user_by_id(&mut conn, id)?;

    let password_hash = hash_password(&body.password)
        .map_err(|e| AppError::Auth(e.to_string()))?;

    user_repo::update_password(&mut conn, id, &password_hash)?;

    Ok(StatusCode::OK)
}
