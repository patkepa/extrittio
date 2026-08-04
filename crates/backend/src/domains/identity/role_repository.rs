use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::role_types::{
    CreateRoleRecord, DeleteRoleOutcome, RoleDetails, UpdateRoleOutcome, UpdateRoleRecord,
};

#[async_trait]
pub trait RoleRepository: Send + Sync {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<RoleDetails>, PersistenceError>;

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateRoleRecord,
    ) -> Result<RoleDetails, PersistenceError>;

    async fn update(
        &self,
        tenant: &TenantId,
        id: i32,
        record: UpdateRoleRecord,
    ) -> Result<UpdateRoleOutcome, PersistenceError>;

    async fn delete(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<DeleteRoleOutcome, PersistenceError>;
}
