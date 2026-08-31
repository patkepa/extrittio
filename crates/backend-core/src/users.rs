use std::fmt;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{Page, PageRequest, PersistenceError, Role, TenantId};

/// An encoded password verifier produced by the host's password implementation.
///
/// This value deliberately implements neither `Serialize` nor a revealing
/// `Debug`. Persistence adapters may expose it only at the credential-specific
/// repository boundary.
#[derive(Clone, PartialEq, Eq)]
pub struct EncodedPasswordHash(String);

impl EncodedPasswordHash {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Debug for EncodedPasswordHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("EncodedPasswordHash")
            .field(&"[REDACTED]")
            .finish()
    }
}

/// Durable, opaque identity generation for a persisted user principal.
///
/// Numeric user IDs are not sufficient session identities because some
/// storage engines may reuse a deleted ID. JWTs bind to this value as well as
/// the numeric ID and permission version so deleting and recreating a user can
/// never revive an older session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAuthEpoch(String);

impl UserAuthEpoch {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i32,
    pub tenant_id: TenantId,
    pub username: String,
    /// Compatibility projection retained while persisted RBAC roles are the
    /// authorization source.
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
    pub permission_version: i32,
    pub auth_epoch: UserAuthEpoch,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDetails {
    pub user: User,
    pub roles: Vec<Role>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCredentials {
    pub details: UserDetails,
    pub password_hash: EncodedPasswordHash,
}

pub type UserPage = Page<UserDetails>;

/// Canonical, validated user data accepted by persistence adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUser {
    pub username: String,
    pub password_hash: EncodedPasswordHash,
    /// `None` selects the tenant's built-in viewer role. `Some` is always
    /// non-empty, sorted, and deduplicated when produced by `UserApplication`.
    pub role_ids: Option<Vec<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateUserOutcome {
    Created(UserDetails),
    RolesNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangePasswordOutcome {
    PasswordChanged,
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteUserOutcome {
    Deleted,
    NotFound,
    WouldDeleteLastOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetUserRolesOutcome {
    Updated(UserDetails),
    UserNotFound,
    RolesNotFound,
    WouldRemoveLastOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordSuccessfulLoginOutcome {
    LoginRecorded,
    NotFound,
}

/// Tenant-scoped users, credentials, and role assignments.
///
/// User pages use bytewise `username ASC, id ASC` order. Hydrated assigned
/// roles use bytewise `name ASC, id ASC`, and effective permission keys use
/// bytewise ascending order. Every write is exact-tenant scoped. User creation,
/// role replacement, and last-owner checks are atomic; concurrent operations
/// cannot delete or demote every owner in a tenant.
/// `find_credentials_by_username` and `get_details` each return one
/// snapshot-consistent identity/role/permission aggregate; adapters must not
/// combine a credential or principal generation with a different revision's
/// authorization state.
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn list(
        &self,
        tenant: &TenantId,
        page: PageRequest,
    ) -> Result<UserPage, PersistenceError>;

    async fn create(
        &self,
        tenant: &TenantId,
        user: NewUser,
    ) -> Result<CreateUserOutcome, PersistenceError>;

    async fn change_password(
        &self,
        tenant: &TenantId,
        user_id: i32,
        password_hash: EncodedPasswordHash,
    ) -> Result<ChangePasswordOutcome, PersistenceError>;

    async fn delete(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<DeleteUserOutcome, PersistenceError>;

    async fn set_roles(
        &self,
        tenant: &TenantId,
        user_id: i32,
        role_ids: Vec<i32>,
    ) -> Result<SetUserRolesOutcome, PersistenceError>;

    async fn find_credentials_by_username(
        &self,
        tenant: &TenantId,
        username: &str,
    ) -> Result<Option<UserCredentials>, PersistenceError>;

    async fn get_details(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<Option<UserDetails>, PersistenceError>;

    async fn record_successful_login(
        &self,
        tenant: &TenantId,
        user_id: i32,
        logged_in_at: DateTime<Utc>,
    ) -> Result<RecordSuccessfulLoginOutcome, PersistenceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_hash_debug_output_is_redacted() {
        let hash = EncodedPasswordHash::new("sensitive-password-verifier");
        let debug = format!("{hash:?}");

        assert_eq!(debug, "EncodedPasswordHash(\"[REDACTED]\")");
        assert!(!debug.contains(hash.as_str()));
    }
}
