use crate::{PersistenceError, TenantId};
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRecord {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetSummary {
    pub fleet: FleetRecord,
    pub device_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetList {
    pub records: Vec<FleetSummary>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateFleetRecord {
    pub name: String,
}

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
