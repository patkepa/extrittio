use chrono::{DateTime, Utc};
use extrittio_backend_core::Role;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRecord {
    pub id: i32,
    pub tenant_id: String,
    pub username: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
    pub permission_version: i32,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDetails {
    pub user: UserRecord,
    pub roles: Vec<Role>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCredentials {
    pub details: UserDetails,
    pub password_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserList {
    pub records: Vec<UserDetails>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateUserRecord {
    pub username: String,
    pub password_hash: String,
    pub role_ids: Option<Vec<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateUserOutcome {
    Created(UserDetails),
    RolesNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetUserRolesOutcome {
    Updated(UserDetails),
    UserNotFound,
    RolesNotFound,
    WouldRemoveLastOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteUserOutcome {
    Deleted,
    NotFound,
    WouldDeleteLastOwner,
}
