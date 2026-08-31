use std::sync::Arc;

use crate::application::require_permission;
use crate::{ADMIN_ROLE, OWNER_ROLE};
use crate::{
    Actor, ApplicationError, ChangePasswordOutcome, Clock, CreateUserOutcome, DeleteUserOutcome,
    EncodedPasswordHash, NewUser, PageRequest, PasswordHasher, Permission,
    RecordSuccessfulLoginOutcome, Role, SetUserRolesOutcome, TenantContext, TenantId,
    UserCredentials, UserDetails, UserPage, UserRepository,
};

pub const MIN_PASSWORD_LEN: usize = 12;

/// Plaintext input for user creation.
///
/// Deliberately does not implement `Debug`, `Clone`, or serialization so a
/// password cannot be copied or logged accidentally through this type.
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub role_ids: Option<Vec<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub id: i32,
    pub tenant_id: TenantId,
    pub username: String,
    pub role: String,
    pub roles: Vec<Role>,
    pub permissions: Vec<String>,
    pub permission_version: i32,
}

#[derive(Clone)]
pub struct UserApplication {
    repository: Arc<dyn UserRepository>,
    password_hasher: Arc<dyn PasswordHasher>,
    clock: Arc<dyn Clock>,
}

impl UserApplication {
    #[must_use]
    pub fn new(
        repository: Arc<dyn UserRepository>,
        password_hasher: Arc<dyn PasswordHasher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            password_hasher,
            clock,
        }
    }

    pub async fn list(
        &self,
        context: &TenantContext,
        page: PageRequest,
    ) -> Result<UserPage, ApplicationError> {
        require_permission(context, Permission::ReadUsers)?;
        Ok(self.repository.list(context.tenant_id(), page).await?)
    }

    pub async fn create(
        &self,
        context: &TenantContext,
        input: CreateUser,
    ) -> Result<UserDetails, ApplicationError> {
        require_permission(context, Permission::ManageUsers)?;
        let username = input.username.trim().to_string();
        if username.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "Username must not be empty".into(),
            ));
        }
        validate_password(&input.password)?;
        if input.role_ids.as_deref().is_some_and(<[i32]>::is_empty) {
            return Err(ApplicationError::InvalidInput(
                "At least one role is required".into(),
            ));
        }

        let role_ids = input.role_ids.map(normalize_role_ids);
        let password_hash = self
            .password_hasher
            .hash(input.password)
            .await
            .map_err(|error| ApplicationError::Authentication(error.to_string()))?;
        let outcome = self
            .repository
            .create(
                context.tenant_id(),
                NewUser {
                    username: username.clone(),
                    password_hash,
                    role_ids,
                },
            )
            .await
            .map_err(|error| map_user_write_error(error, &username))?;
        match outcome {
            CreateUserOutcome::Created(user) => Ok(user),
            CreateUserOutcome::RolesNotFound => Err(ApplicationError::NotFound(
                "One or more roles were not found".into(),
            )),
        }
    }

    pub async fn change_password(
        &self,
        context: &TenantContext,
        user_id: i32,
        new_password: String,
    ) -> Result<(), ApplicationError> {
        require_permission(context, Permission::ManageUsers)?;
        validate_password(&new_password)?;
        let password_hash = self
            .password_hasher
            .hash(new_password)
            .await
            .map_err(|error| ApplicationError::Authentication(error.to_string()))?;
        match self
            .repository
            .change_password(context.tenant_id(), user_id, password_hash)
            .await?
        {
            ChangePasswordOutcome::PasswordChanged => Ok(()),
            ChangePasswordOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "User {user_id} not found"
            ))),
        }
    }

    pub async fn delete(
        &self,
        context: &TenantContext,
        user_id: i32,
    ) -> Result<(), ApplicationError> {
        require_permission(context, Permission::ManageUsers)?;
        match self.repository.delete(context.tenant_id(), user_id).await? {
            DeleteUserOutcome::Deleted => Ok(()),
            DeleteUserOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "User {user_id} not found"
            ))),
            DeleteUserOutcome::WouldDeleteLastOwner => Err(ApplicationError::Conflict(
                "Cannot delete the last owner from the tenant".into(),
            )),
        }
    }

    pub async fn set_roles(
        &self,
        context: &TenantContext,
        user_id: i32,
        role_ids: Vec<i32>,
    ) -> Result<UserDetails, ApplicationError> {
        require_permission(context, Permission::ManageUsers)?;
        if role_ids.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "At least one role is required".into(),
            ));
        }
        match self
            .repository
            .set_roles(context.tenant_id(), user_id, normalize_role_ids(role_ids))
            .await?
        {
            SetUserRolesOutcome::Updated(user) => Ok(user),
            SetUserRolesOutcome::UserNotFound => Err(ApplicationError::NotFound(format!(
                "User {user_id} not found"
            ))),
            SetUserRolesOutcome::RolesNotFound => Err(ApplicationError::NotFound(
                "One or more roles were not found".into(),
            )),
            SetUserRolesOutcome::WouldRemoveLastOwner => Err(ApplicationError::Conflict(
                "Cannot remove the last owner from the tenant".into(),
            )),
        }
    }

    /// Authenticate an exact username within an explicitly selected tenant.
    /// JWT issuance and default-tenant login compatibility remain host-owned.
    pub async fn authenticate(
        &self,
        tenant: &TenantId,
        username: &str,
        password: String,
    ) -> Result<AuthenticatedUser, ApplicationError> {
        let credentials = self
            .repository
            .find_credentials_by_username(tenant, username)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        let UserCredentials {
            details,
            password_hash,
        } = credentials;
        if !details.user.is_active {
            return Err(ApplicationError::Unauthorized);
        }
        if !self.verify_password(password, password_hash).await? {
            return Err(ApplicationError::Unauthorized);
        }

        // Preserve the existing race behavior: once credentials verify, a
        // concurrent delete that makes this best-effort timestamp update miss
        // does not change the authentication result.
        let _outcome = self
            .repository
            .record_successful_login(tenant, details.user.id, self.clock.now())
            .await?;
        Ok(authenticated_user_from_details(details))
    }

    /// Resolve current persisted authorization for an already validated host
    /// credential without admitting JWT types into core.
    pub async fn resolve_session(
        &self,
        tenant: &TenantId,
        user_id: i32,
        permission_version: i32,
    ) -> Result<AuthenticatedUser, ApplicationError> {
        let details = self
            .repository
            .get_details(tenant, user_id)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if !matches_identity(&details, tenant, user_id)
            || !details.user.is_active
            || details.user.permission_version != permission_version
        {
            return Err(ApplicationError::Unauthorized);
        }
        Ok(authenticated_user_from_details(details))
    }

    pub async fn current_user(
        &self,
        context: &TenantContext,
    ) -> Result<AuthenticatedUser, ApplicationError> {
        let Actor::User { id: user_id, .. } = context.actor() else {
            return Err(ApplicationError::Unauthorized);
        };
        let details = self
            .repository
            .get_details(context.tenant_id(), *user_id)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if !matches_identity(&details, context.tenant_id(), *user_id) || !details.user.is_active {
            return Err(ApplicationError::Unauthorized);
        }
        Ok(authenticated_user_from_details(details))
    }

    async fn verify_password(
        &self,
        password: String,
        password_hash: EncodedPasswordHash,
    ) -> Result<bool, ApplicationError> {
        self.password_hasher
            .verify(password, password_hash)
            .await
            .map_err(|error| {
                ApplicationError::Internal(format!("password verification failed: {error}"))
            })
    }
}

#[must_use]
pub fn authenticated_user_from_details(details: UserDetails) -> AuthenticatedUser {
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

#[must_use]
pub fn primary_role_name(roles: &[Role]) -> Option<&str> {
    roles
        .iter()
        .find(|role| role.name == OWNER_ROLE)
        .or_else(|| roles.iter().find(|role| role.name == ADMIN_ROLE))
        .or_else(|| roles.first())
        .map(|role| role.name.as_str())
}

pub fn validate_password(password: &str) -> Result<(), ApplicationError> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(ApplicationError::InvalidInput(format!(
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
        return Err(ApplicationError::InvalidInput(
            "Password must include lowercase, uppercase, number, and symbol characters".into(),
        ));
    }
    Ok(())
}

fn normalize_role_ids(mut role_ids: Vec<i32>) -> Vec<i32> {
    role_ids.sort_unstable();
    role_ids.dedup();
    role_ids
}

fn matches_identity(details: &UserDetails, tenant: &TenantId, user_id: i32) -> bool {
    details.user.id == user_id && &details.user.tenant_id == tenant
}

fn map_user_write_error(error: crate::PersistenceError, username: &str) -> ApplicationError {
    match error {
        crate::PersistenceError::UniqueViolation { .. } => {
            ApplicationError::Conflict(format!("Username '{username}' already exists"))
        }
        other => ApplicationError::Persistence(other),
    }
}
