use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{PersistenceError, TenantId};

pub const OWNER_ROLE: &str = "owner";
pub const ADMIN_ROLE: &str = "admin";
pub const OPERATOR_ROLE: &str = "operator";
pub const VIEWER_ROLE: &str = "viewer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub id: i32,
    pub tenant_id: TenantId,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleDetails {
    pub role: Role,
    pub permissions: Vec<String>,
    pub user_count: i64,
}

/// Canonical, validated role data accepted by persistence adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRole {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

/// Atomic role mutation. `Some(permissions)` replaces every permission and
/// invalidates all assigned-user permission versions in the same transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolePatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateRoleOutcome {
    Updated(RoleDetails),
    NotFound,
    SystemRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteRoleOutcome {
    Deleted,
    NotFound,
    SystemRole,
    InUse { name: String, user_count: i64 },
}

/// Tenant-scoped roles and permission assignments.
///
/// Implementations return system roles first, then bytewise name and ID order.
/// Permission keys within each role are bytewise ordered. Updates and deletes
/// perform their read/check/write behavior atomically. Every successful update
/// advances `updated_at` once using the adapter clock, including an empty patch.
#[async_trait]
pub trait RoleRepository: Send + Sync {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<RoleDetails>, PersistenceError>;

    async fn create(
        &self,
        tenant: &TenantId,
        role: NewRole,
    ) -> Result<RoleDetails, PersistenceError>;

    async fn update(
        &self,
        tenant: &TenantId,
        role_id: i32,
        patch: RolePatch,
    ) -> Result<UpdateRoleOutcome, PersistenceError>;

    async fn delete(
        &self,
        tenant: &TenantId,
        role_id: i32,
    ) -> Result<DeleteRoleOutcome, PersistenceError>;
}
