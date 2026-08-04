use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{CreateFleetRecord, FleetList, FleetRecord};

#[async_trait]
pub trait FleetRepository: Send + Sync {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<FleetList, PersistenceError>;

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateFleetRecord,
    ) -> Result<FleetRecord, PersistenceError>;

    async fn rename(
        &self,
        tenant: &TenantId,
        id: i32,
        name: String,
    ) -> Result<Option<FleetRecord>, PersistenceError>;

    async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError>;
}
