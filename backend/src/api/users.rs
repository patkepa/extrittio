use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::hash_password;
use crate::db::models::NewUser;
use crate::error::AppError;
use crate::pagination::{self, PaginatedResponse, PaginationParams};
use crate::repositories::user_repo;
use crate::state::{run_db, AppState};

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
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserResponse>>, AppError> {
    let (limit, offset) = pagination::clamp(params.limit, params.offset);

    let response = run_db(&state.db_pool, move |conn| {
        let (results, total) = user_repo::list_users(conn, limit, offset)?;
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

async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    if body.username.trim().is_empty() {
        return Err(AppError::BadRequest("Username must not be empty".into()));
    }
    if body.password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 4 characters".into(),
        ));
    }

    let password_hash =
        hash_password(&body.password).map_err(|e| AppError::Auth(e.to_string()))?;

    let username = body.username;

    let user = run_db(&state.db_pool, move |conn| {
        let new_user = NewUser {
            username: username.clone(),
            password_hash,
        };

        user_repo::insert_user(conn, &new_user).map_err(|e| match e {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ) => AppError::Conflict(format!("Username '{username}' already exists")),
            other => AppError::Database(other),
        })
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

async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    run_db(&state.db_pool, move |conn| {
        let deleted = user_repo::delete_user(conn, id)?;
        if !deleted {
            return Err(AppError::NotFound(format!("User {id} not found")));
        }
        Ok(())
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn change_password(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<StatusCode, AppError> {
    if body.password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 4 characters".into(),
        ));
    }

    let password_hash =
        hash_password(&body.password).map_err(|e| AppError::Auth(e.to_string()))?;

    run_db(&state.db_pool, move |conn| {
        // Verify user exists
        user_repo::find_user_by_id(conn, id)?;
        user_repo::update_password(conn, id, &password_hash)?;
        Ok(())
    })
    .await?;

    Ok(StatusCode::OK)
}
