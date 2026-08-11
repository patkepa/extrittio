use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{DeleteZoneOutcome, NewZoneRecord, UpdateZoneRecord, ZoneRecord};

#[async_trait]
pub trait ZoneRepository: Send + Sync {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<ZoneRecord>, PersistenceError>;
    async fn list_all(&self) -> Result<Vec<ZoneRecord>, PersistenceError>;
    async fn get(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<Option<ZoneRecord>, PersistenceError>;
    async fn create(
        &self,
        tenant: &TenantId,
        record: NewZoneRecord,
    ) -> Result<ZoneRecord, PersistenceError>;
    async fn update(
        &self,
        tenant: &TenantId,
        zone_id: &str,
        record: UpdateZoneRecord,
    ) -> Result<Option<ZoneRecord>, PersistenceError>;
    async fn delete(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<DeleteZoneOutcome, PersistenceError>;
}
