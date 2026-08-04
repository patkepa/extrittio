use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{
    CreateDeviceTypeRecord, DeleteDeviceTypeOutcome, DeviceTypeList, DeviceTypeRecord,
    UpdateDeviceTypeRecord,
};

#[async_trait]
pub trait DeviceTypeRepository: Send + Sync {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<DeviceTypeList, PersistenceError>;

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceTypeRecord,
    ) -> Result<DeviceTypeRecord, PersistenceError>;

    async fn update(
        &self,
        tenant: &TenantId,
        id: i32,
        record: UpdateDeviceTypeRecord,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError>;

    async fn get_by_id(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError>;

    /// Atomically verifies that no devices reference the type and deletes it.
    async fn delete_if_unused(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<DeleteDeviceTypeOutcome, PersistenceError>;
}
