use diesel::PgConnection;
use std::collections::HashSet;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{NewRole, Role};
use crate::error::AppError;
use crate::repositories::{role_repo, user_repo};

pub const OWNER_ROLE: &str = "owner";
pub const ADMIN_ROLE: &str = "admin";
pub const OPERATOR_ROLE: &str = "operator";
pub const VIEWER_ROLE: &str = "viewer";

#[derive(Debug, Clone)]
pub struct RoleWithPermissions {
    pub role: Role,
    pub permissions: Vec<String>,
    pub user_count: i64,
}

pub fn list(
    ctx: &RequestContext,
    conn: &mut PgConnection,
) -> Result<Vec<RoleWithPermissions>, AppError> {
    policy::require(ctx, Permission::ReadRoles)?;
    list_for_tenant(conn, ctx.tenant_id_str())
}

pub fn list_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
) -> Result<Vec<RoleWithPermissions>, AppError> {
    let roles = role_repo::list_roles(conn, tenant_id)?;
    roles
        .into_iter()
        .map(|role| hydrate_role(conn, tenant_id, role))
        .collect()
}

pub fn create(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    name: &str,
    description: Option<String>,
    permissions: &[String],
) -> Result<RoleWithPermissions, AppError> {
    policy::require(ctx, Permission::ManageRoles)?;

    let name = normalize_role_name(name)?;
    if is_builtin_role_name(&name) {
        return Err(AppError::BadRequest(format!(
            "'{name}' is reserved for a built-in role"
        )));
    }
    let permissions = validate_permission_keys(permissions)?;

    let role = role_repo::insert_role(
        conn,
        &NewRole {
            tenant_id: ctx.tenant_id_str().to_string(),
            name,
            description,
            is_system: false,
        },
    )
    .map_err(map_unique_role_error)?;

    role_repo::replace_role_permissions(conn, role.id, &permissions)?;
    hydrate_role(conn, ctx.tenant_id_str(), role)
}

pub fn update(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    id: i32,
    name: Option<String>,
    description: Option<Option<String>>,
    permissions: Option<Vec<String>>,
) -> Result<RoleWithPermissions, AppError> {
    policy::require(ctx, Permission::ManageRoles)?;

    let existing = role_repo::find_role(conn, ctx.tenant_id_str(), id)?;
    if existing.is_system {
        return Err(AppError::BadRequest(
            "Built-in roles cannot be modified".into(),
        ));
    }

    let next_name = match name {
        Some(name) => normalize_role_name(&name)?,
        None => existing.name.clone(),
    };
    if is_builtin_role_name(&next_name) {
        return Err(AppError::BadRequest(format!(
            "'{next_name}' is reserved for a built-in role"
        )));
    }

    let next_description = description.unwrap_or(existing.description.clone());
    let role = role_repo::update_role(
        conn,
        ctx.tenant_id_str(),
        id,
        &next_name,
        next_description.as_deref(),
    )
    .map_err(map_unique_role_error)?;

    if let Some(permissions) = permissions {
        let permissions = validate_permission_keys(&permissions)?;
        role_repo::replace_role_permissions(conn, role.id, &permissions)?;
    }

    hydrate_role(conn, ctx.tenant_id_str(), role)
}

pub fn delete(ctx: &RequestContext, conn: &mut PgConnection, id: i32) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageRoles)?;

    let role = role_repo::find_role(conn, ctx.tenant_id_str(), id)?;
    if role.is_system {
        return Err(AppError::BadRequest(
            "Built-in roles cannot be deleted".into(),
        ));
    }

    let user_count = role_repo::count_users_for_role(conn, ctx.tenant_id_str(), id)?;
    if user_count > 0 {
        return Err(AppError::Conflict(format!(
            "Role '{}' is assigned to {user_count} user(s)",
            role.name
        )));
    }

    if !role_repo::delete_role(conn, ctx.tenant_id_str(), id)? {
        return Err(AppError::NotFound(format!("Role {id} not found")));
    }

    Ok(())
}

pub fn roles_for_user(
    conn: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
) -> Result<Vec<Role>, AppError> {
    Ok(role_repo::roles_for_user(conn, tenant_id, user_id)?)
}

pub fn assign_roles_to_user(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    user_id: i32,
    role_ids: &[i32],
) -> Result<Vec<Role>, AppError> {
    policy::require(ctx, Permission::ManageUsers)?;
    assign_roles_to_user_for_tenant(conn, ctx.tenant_id_str(), user_id, role_ids)
}

pub fn assign_roles_to_user_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
    role_ids: &[i32],
) -> Result<Vec<Role>, AppError> {
    if role_ids.is_empty() {
        return Err(AppError::BadRequest("At least one role is required".into()));
    }

    user_repo::find_user_by_id(conn, tenant_id, user_id)?;

    let next_roles = role_repo::find_roles_by_ids(conn, tenant_id, role_ids)?;
    let unique_count = role_ids.iter().copied().collect::<HashSet<_>>().len();
    if next_roles.len() != unique_count {
        return Err(AppError::NotFound(
            "One or more roles were not found".into(),
        ));
    }

    let currently_owner = role_repo::user_has_role_name(conn, tenant_id, user_id, OWNER_ROLE)?;
    let remains_owner = next_roles.iter().any(|role| role.name == OWNER_ROLE);
    if currently_owner && !remains_owner {
        ensure_not_last_owner(conn, tenant_id)?;
    }

    Ok(role_repo::set_user_roles(
        conn, tenant_id, user_id, role_ids,
    )?)
}

pub fn default_viewer_role(conn: &mut PgConnection, tenant_id: &str) -> Result<Role, AppError> {
    Ok(role_repo::find_role_by_name(conn, tenant_id, VIEWER_ROLE)?)
}

pub fn owner_role(conn: &mut PgConnection, tenant_id: &str) -> Result<Role, AppError> {
    Ok(role_repo::find_role_by_name(conn, tenant_id, OWNER_ROLE)?)
}

pub fn permission_keys_for_user(
    conn: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
) -> Result<Vec<String>, AppError> {
    Ok(role_repo::permissions_for_user(conn, tenant_id, user_id)?)
}

fn hydrate_role(
    conn: &mut PgConnection,
    tenant_id: &str,
    role: Role,
) -> Result<RoleWithPermissions, AppError> {
    let permissions = role_repo::permissions_for_role(conn, role.id)?;
    let user_count = role_repo::count_users_for_role(conn, tenant_id, role.id)?;
    Ok(RoleWithPermissions {
        role,
        permissions,
        user_count,
    })
}

fn ensure_not_last_owner(conn: &mut PgConnection, tenant_id: &str) -> Result<(), AppError> {
    let owner_count = role_repo::count_users_with_role_name(conn, tenant_id, OWNER_ROLE)?;
    if owner_count <= 1 {
        Err(AppError::Conflict(
            "Cannot remove the last owner from the tenant".into(),
        ))
    } else {
        Ok(())
    }
}

fn normalize_role_name(name: &str) -> Result<String, AppError> {
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Err(AppError::BadRequest("Role name must not be empty".into()));
    }
    if name.len() > 64 {
        return Err(AppError::BadRequest(
            "Role name must be 64 characters or fewer".into(),
        ));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppError::BadRequest(
            "Role name may only contain lowercase letters, numbers, hyphens, and underscores"
                .into(),
        ));
    }
    Ok(name)
}

fn validate_permission_keys(keys: &[String]) -> Result<Vec<String>, AppError> {
    let mut permissions = keys
        .iter()
        .map(|key| key.trim())
        .filter(|key| !key.is_empty())
        .map(|key| {
            Permission::from_key(key)
                .map(|permission| permission.key().to_string())
                .ok_or_else(|| AppError::BadRequest(format!("Unknown permission '{key}'")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    permissions.sort();
    permissions.dedup();
    Ok(permissions)
}

fn is_builtin_role_name(name: &str) -> bool {
    matches!(name, OWNER_ROLE | ADMIN_ROLE | OPERATOR_ROLE | VIEWER_ROLE)
}

fn map_unique_role_error(error: diesel::result::Error) -> AppError {
    match error {
        diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _,
        ) => AppError::Conflict("Role name already exists".into()),
        other => AppError::Database(other),
    }
}
