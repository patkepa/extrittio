use crate::{PersistenceError, TenantId};
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceTypeRecord {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub color_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceTypeList {
    pub records: Vec<DeviceTypeRecord>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateDeviceTypeRecord {
    pub name: String,
    pub icon: String,
    pub color_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateDeviceTypeRecord {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteDeviceTypeOutcome {
    Deleted,
    NotFound,
    InUse { device_count: i64 },
}

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

    async fn get_by_name(
        &self,
        tenant: &TenantId,
        name: &str,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError>;

    /// Atomically verifies that no devices reference the type and deletes it.
    async fn delete_if_unused(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<DeleteDeviceTypeOutcome, PersistenceError>;
}
