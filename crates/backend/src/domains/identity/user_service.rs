use crate::auth::Claims;
use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::auth::{hash_password, verify_password};
use crate::domains::identity::role_types::RoleRecord;
use crate::domains::identity::user_repository::UserRepository;
use crate::domains::identity::user_types::{
    CreateUserOutcome, CreateUserRecord, DeleteUserOutcome, SetUserRolesOutcome, UserDetails,
    UserList,
};
use crate::error::AppError;
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

pub const MIN_PASSWORD_LEN: usize = 12;

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: i32,
    pub tenant_id: String,
    pub username: String,
    pub role: String,
    pub roles: Vec<RoleRecord>,
    pub permissions: Vec<String>,
    pub permission_version: i32,
}

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn UserRepository,
    limit: i64,
    offset: i64,
) -> Result<UserList, AppError> {
    policy::require(ctx, Permission::ReadUsers)?;
    Ok(repository.list(ctx.tenant_id(), limit, offset).await?)
}

pub async fn create(
    ctx: &RequestContext,
    repository: &dyn UserRepository,
    username: &str,
    password: &str,
    role_ids: Option<&[i32]>,
) -> Result<UserDetails, AppError> {
    policy::require(ctx, Permission::ManageUsers)?;
    let username = username.trim();
    if username.is_empty() {
        return Err(AppError::BadRequest("Username must not be empty".into()));
    }
    validate_password(password)?;
    if role_ids.is_some_and(<[i32]>::is_empty) {
        return Err(AppError::BadRequest("At least one role is required".into()));
    }

    let password_hash = hash_password_async(password.to_string()).await?;
    let outcome = repository
        .create(
            ctx.tenant_id(),
            CreateUserRecord {
                username: username.to_string(),
                password_hash,
                role_ids: role_ids.map(<[i32]>::to_vec),
            },
        )
        .await
        .map_err(|error| map_user_write_error(error, username))?;
    match outcome {
        CreateUserOutcome::Created(user) => Ok(user),
        CreateUserOutcome::RolesNotFound => Err(AppError::NotFound(
            "One or more roles were not found".into(),
        )),
    }
}

pub async fn change_password(
    ctx: &RequestContext,
    repository: &dyn UserRepository,
    user_id: i32,
    new_password: &str,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageUsers)?;
    validate_password(new_password)?;
    let password_hash = hash_password_async(new_password.to_string()).await?;
    if repository
        .change_password(ctx.tenant_id(), user_id, password_hash)
        .await?
    {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("User {user_id} not found")))
    }
}

pub async fn delete(
    ctx: &RequestContext,
    repository: &dyn UserRepository,
    id: i32,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageUsers)?;
    match repository.delete(ctx.tenant_id(), id).await? {
        DeleteUserOutcome::Deleted => Ok(()),
        DeleteUserOutcome::NotFound => Err(AppError::NotFound(format!("User {id} not found"))),
        DeleteUserOutcome::WouldDeleteLastOwner => Err(AppError::Conflict(
            "Cannot delete the last owner from the tenant".into(),
        )),
    }
}

pub async fn authenticate(
    repository: &dyn UserRepository,
    tenant: &TenantId,
    username: &str,
    password: &str,
) -> Result<AuthenticatedUser, AppError> {
    let credentials = repository
        .find_credentials_by_username(tenant, username)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !credentials.details.user.is_active {
        return Err(AppError::Unauthorized);
    }
    if !verify_password_async(password.to_string(), credentials.password_hash).await? {
        return Err(AppError::Unauthorized);
    }

    let _ = repository
        .record_successful_login(tenant, credentials.details.user.id, chrono::Utc::now())
        .await?;
    Ok(authenticated_user_from_details(credentials.details))
}

pub async fn context_from_claims(
    repository: &dyn UserRepository,
    claims: Claims,
) -> Result<RequestContext, AppError> {
    let base_context = RequestContext::from_claims(claims.clone());
    let details = repository
        .get_details(base_context.tenant_id(), claims.sub)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !details.user.is_active || details.user.permission_version != claims.permission_version {
        return Err(AppError::Unauthorized);
    }

    let authenticated = authenticated_user_from_details(details);
    let permissions = Permission::from_keys(&authenticated.permissions);
    Ok(RequestContext {
        user_id: authenticated.id,
        username: authenticated.username,
        role: authenticated.role,
        tenant_id: base_context.tenant_id,
        scopes: authenticated.permissions,
        permissions,
    })
}

pub async fn set_roles(
    ctx: &RequestContext,
    repository: &dyn UserRepository,
    user_id: i32,
    role_ids: &[i32],
) -> Result<UserDetails, AppError> {
    policy::require(ctx, Permission::ManageUsers)?;
    if role_ids.is_empty() {
        return Err(AppError::BadRequest("At least one role is required".into()));
    }
    match repository
        .set_roles(ctx.tenant_id(), user_id, role_ids.to_vec())
        .await?
    {
        SetUserRolesOutcome::Updated(user) => Ok(user),
        SetUserRolesOutcome::UserNotFound => {
            Err(AppError::NotFound(format!("User {user_id} not found")))
        }
        SetUserRolesOutcome::RolesNotFound => Err(AppError::NotFound(
            "One or more roles were not found".into(),
        )),
        SetUserRolesOutcome::WouldRemoveLastOwner => Err(AppError::Conflict(
            "Cannot remove the last owner from the tenant".into(),
        )),
    }
}

pub async fn current_user(
    ctx: &RequestContext,
    repository: &dyn UserRepository,
) -> Result<AuthenticatedUser, AppError> {
    let details = repository
        .get_details(ctx.tenant_id(), ctx.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !details.user.is_active {
        return Err(AppError::Unauthorized);
    }
    Ok(authenticated_user_from_details(details))
}

fn authenticated_user_from_details(details: UserDetails) -> AuthenticatedUser {
    let role = primary_role_name(&details.roles)
        .unwrap_or(&details.user.role)
        .to_string();
    AuthenticatedUser {
        id: details.user.id,
        tenant_id: details.user.tenant_id,
        username: details.user.username,
        role,
        roles: details.roles,
        permissions: details.permissions,
        permission_version: details.user.permission_version,
    }
}

fn primary_role_name(roles: &[RoleRecord]) -> Option<&str> {
    roles
        .iter()
        .find(|role| role.name == super::role_service::OWNER_ROLE)
        .or_else(|| {
            roles
                .iter()
                .find(|role| role.name == super::role_service::ADMIN_ROLE)
        })
        .or_else(|| roles.first())
        .map(|role| role.name.as_str())
}

async fn hash_password_async(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|error| AppError::Internal(format!("password task failed: {error}")))?
        .map_err(|error| AppError::Auth(error.to_string()))
}

async fn verify_password_async(password: String, hash: String) -> Result<bool, AppError> {
    tokio::task::spawn_blocking(move || verify_password(&password, &hash))
        .await
        .map_err(|error| AppError::Internal(format!("password task failed: {error}")))
}

fn map_user_write_error(error: PersistenceError, username: &str) -> AppError {
    match error {
        PersistenceError::UniqueViolation { .. } => {
            AppError::Conflict(format!("Username '{username}' already exists"))
        }
        other => AppError::Persistence(other),
    }
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::BadRequest(format!(
            "Password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }

    let has_lower = password
        .chars()
        .any(|character| character.is_ascii_lowercase());
    let has_upper = password
        .chars()
        .any(|character| character.is_ascii_uppercase());
    let has_digit = password.chars().any(|character| character.is_ascii_digit());
    let has_symbol = password
        .chars()
        .any(|character| !character.is_ascii_alphanumeric());

    if !(has_lower && has_upper && has_digit && has_symbol) {
        return Err(AppError::BadRequest(
            "Password must include lowercase, uppercase, number, and symbol characters".into(),
        ));
    }
    Ok(())
}
