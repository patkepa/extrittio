use diesel::Connection;
use diesel::PgConnection;

use crate::auth::Claims;
use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::auth::{hash_password, verify_password};
use crate::db::models::{NewUser, Role, User};
use crate::error::AppError;
use crate::repositories::{role_repo, user_repo};
use crate::services::role_service;
use crate::tenancy::DEFAULT_TENANT_ID;

#[derive(Debug, Clone)]
pub struct UserWithRoles {
    pub user: User,
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: i32,
    pub tenant_id: String,
    pub username: String,
    pub role: String,
    pub roles: Vec<Role>,
    pub permissions: Vec<String>,
    pub permission_version: i32,
}

pub fn list(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<UserWithRoles>, i64), AppError> {
    policy::require(ctx, Permission::ReadUsers)?;
    let (users, total) = user_repo::list_users(conn, ctx.tenant_id_str(), limit, offset)?;
    let users = users
        .into_iter()
        .map(|user| {
            let roles = role_service::roles_for_user(conn, ctx.tenant_id_str(), user.id)?;
            Ok(UserWithRoles { user, roles })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok((users, total))
}

pub fn create(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    username: &str,
    password: &str,
    role_ids: Option<&[i32]>,
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

    let tenant_id = ctx.tenant_id_str().to_string();
    conn.transaction(|conn| {
        let assigned_roles = resolve_roles_for_new_user(conn, &tenant_id, role_ids)?;
        let primary_role = primary_role_name(&assigned_roles).unwrap_or(role_service::VIEWER_ROLE);
        let password_hash = hash_password(password).map_err(|e| AppError::Auth(e.to_string()))?;
        let new_user = NewUser {
            tenant_id: tenant_id.clone(),
            username: username.to_string(),
            password_hash,
            role: primary_role.to_string(),
        };

        let user = user_repo::insert_user(conn, &new_user).map_err(|e| match e {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ) => AppError::Conflict(format!("Username '{username}' already exists")),
            other => AppError::Database(other),
        })?;

        let role_ids: Vec<i32> = assigned_roles.iter().map(|role| role.id).collect();
        role_repo::set_user_roles(conn, &tenant_id, user.id, &role_ids)?;
        user_repo::find_user_by_id(conn, &tenant_id, user.id).map_err(AppError::Database)
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

    if role_repo::user_has_role_name(conn, ctx.tenant_id_str(), id, role_service::OWNER_ROLE)?
        && role_repo::count_users_with_role_name(
            conn,
            ctx.tenant_id_str(),
            role_service::OWNER_ROLE,
        )? <= 1
    {
        return Err(AppError::Conflict(
            "Cannot delete the last owner from the tenant".into(),
        ));
    }

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
    tenant_id: &str,
    username: &str,
    password: &str,
) -> Result<AuthenticatedUser, AppError> {
    let user = user_repo::find_user_by_username(conn, tenant_id, username).map_err(|e| match e {
        diesel::result::Error::NotFound => AppError::Unauthorized,
        other => AppError::Database(other),
    })?;

    if !user.is_active || !verify_password(password, &user.password_hash) {
        return Err(AppError::Unauthorized);
    }

    user_repo::update_last_login_at(conn, tenant_id, user.id)?;
    authenticated_user_from_user(conn, user)
}

pub fn context_from_claims(
    conn: &mut PgConnection,
    claims: Claims,
) -> Result<RequestContext, AppError> {
    let base_ctx = RequestContext::from_claims(claims.clone());
    let user = user_repo::find_user_by_id(conn, base_ctx.tenant_id_str(), claims.sub).map_err(
        |e| match e {
            diesel::result::Error::NotFound => AppError::Unauthorized,
            other => AppError::Database(other),
        },
    )?;

    if !user.is_active {
        return Err(AppError::Unauthorized);
    }
    if user.permission_version != claims.permission_version {
        return Err(AppError::Unauthorized);
    }

    let authenticated = authenticated_user_from_user(conn, user)?;
    let permissions = Permission::from_keys(&authenticated.permissions);

    Ok(RequestContext {
        user_id: authenticated.id,
        username: authenticated.username,
        role: authenticated.role,
        tenant_id: base_ctx.tenant_id,
        scopes: authenticated.permissions,
        permissions,
    })
}

pub fn set_roles(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    user_id: i32,
    role_ids: &[i32],
) -> Result<UserWithRoles, AppError> {
    let roles = role_service::assign_roles_to_user(ctx, conn, user_id, role_ids)?;
    let user = user_repo::find_user_by_id(conn, ctx.tenant_id_str(), user_id)?;
    Ok(UserWithRoles { user, roles })
}

pub fn current_user(
    ctx: &RequestContext,
    conn: &mut PgConnection,
) -> Result<AuthenticatedUser, AppError> {
    let user = user_repo::find_user_by_id(conn, ctx.tenant_id_str(), ctx.user_id).map_err(|e| {
        match e {
            diesel::result::Error::NotFound => AppError::Unauthorized,
            other => AppError::Database(other),
        }
    })?;

    if !user.is_active {
        return Err(AppError::Unauthorized);
    }

    authenticated_user_from_user(conn, user)
}

fn authenticated_user_from_user(
    conn: &mut PgConnection,
    user: User,
) -> Result<AuthenticatedUser, AppError> {
    let roles = role_service::roles_for_user(conn, &user.tenant_id, user.id)?;
    let mut permissions = role_service::permission_keys_for_user(conn, &user.tenant_id, user.id)?;

    if permissions.is_empty() && matches!(user.role.as_str(), "admin" | "owner") {
        permissions = Permission::all()
            .iter()
            .map(|permission| permission.key().to_string())
            .collect();
    }

    let role = primary_role_name(&roles).unwrap_or(&user.role).to_string();

    Ok(AuthenticatedUser {
        id: user.id,
        tenant_id: user.tenant_id,
        username: user.username,
        role,
        roles,
        permissions,
        permission_version: user.permission_version,
    })
}

fn resolve_roles_for_new_user(
    conn: &mut PgConnection,
    tenant_id: &str,
    role_ids: Option<&[i32]>,
) -> Result<Vec<Role>, AppError> {
    match role_ids {
        Some(role_ids) if role_ids.is_empty() => {
            Err(AppError::BadRequest("At least one role is required".into()))
        }
        Some(role_ids) => {
            let roles = role_repo::find_roles_by_ids(conn, tenant_id, role_ids)?;
            let unique_count = role_ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len();
            if roles.len() != unique_count {
                return Err(AppError::NotFound(
                    "One or more roles were not found".into(),
                ));
            }
            Ok(roles)
        }
        None => Ok(vec![role_service::default_viewer_role(conn, tenant_id)?]),
    }
}

fn primary_role_name(roles: &[Role]) -> Option<&str> {
    roles
        .iter()
        .find(|role| role.name == role_service::OWNER_ROLE)
        .or_else(|| {
            roles
                .iter()
                .find(|role| role.name == role_service::ADMIN_ROLE)
        })
        .or_else(|| roles.first())
        .map(|role| role.name.as_str())
}
