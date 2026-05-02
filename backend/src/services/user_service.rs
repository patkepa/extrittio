use diesel::PgConnection;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::auth::{hash_password, verify_password};
use crate::db::models::{NewUser, User};
use crate::error::AppError;
use crate::repositories::user_repo;
use crate::tenancy::DEFAULT_TENANT_ID;

pub fn list(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<User>, i64), AppError> {
    policy::require(ctx, Permission::ReadUsers)?;
    Ok(user_repo::list_users(
        conn,
        ctx.tenant_id_str(),
        limit,
        offset,
    )?)
}

pub fn create(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    username: &str,
    password: &str,
) -> Result<User, AppError> {
    policy::require(ctx, Permission::ManageUsers)?;

    if username.trim().is_empty() {
        return Err(AppError::BadRequest("Username must not be empty".into()));
    }
    if password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 4 characters".into(),
        ));
    }

    let password_hash = hash_password(password).map_err(|e| AppError::Auth(e.to_string()))?;
    let new_user = NewUser {
        tenant_id: ctx.tenant_id_str().to_string(),
        username: username.to_string(),
        password_hash,
    };

    user_repo::insert_user(conn, &new_user).map_err(|e| match e {
        diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _,
        ) => AppError::Conflict(format!("Username '{username}' already exists")),
        other => AppError::Database(other),
    })
}

pub fn change_password(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    user_id: i32,
    new_password: &str,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageUsers)?;

    if new_password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 4 characters".into(),
        ));
    }

    user_repo::find_user_by_id(conn, ctx.tenant_id_str(), user_id)?;

    let password_hash = hash_password(new_password).map_err(|e| AppError::Auth(e.to_string()))?;
    user_repo::update_password(conn, ctx.tenant_id_str(), user_id, &password_hash)?;
    Ok(())
}

pub fn delete(ctx: &RequestContext, conn: &mut PgConnection, id: i32) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageUsers)?;

    let deleted = user_repo::delete_user_for_tenant(conn, ctx.tenant_id_str(), id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("User {id} not found")));
    }
    Ok(())
}

/// Authenticate a user by username and password.
/// Returns (user_id, username, role) on success.
pub fn authenticate(
    conn: &mut PgConnection,
    username: &str,
    password: &str,
) -> Result<(i32, String, String), AppError> {
    let user = user_repo::find_user_by_username(conn, DEFAULT_TENANT_ID, username).map_err(
        |e| match e {
            diesel::result::Error::NotFound => AppError::Unauthorized,
            other => AppError::Database(other),
        },
    )?;

    if !verify_password(password, &user.password_hash) {
        return Err(AppError::Unauthorized);
    }

    Ok((user.id, user.username, user.role))
}
