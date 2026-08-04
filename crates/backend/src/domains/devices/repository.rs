use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domains::identity::certificate_types::NewDeviceCertificateRecord;
use crate::persistence::PersistenceError;
use crate::tenancy::DeviceIdentity;
use crate::tenancy::TenantId;

use super::types::{
    CreateDeviceRecord, DeviceDetails, DeviceFilter, DeviceList, DeviceListQuery,
    UpdateDeviceRecord,
};

#[async_trait]
pub trait DeviceRepository: Send + Sync {
    /// Resolves the globally unique protocol device ID to its tenant-qualified
    /// identity before any tenant-owned ingress operation runs.
    async fn resolve_identity(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceIdentity>, PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        query: DeviceListQuery,
    ) -> Result<DeviceList, PersistenceError>;

    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceDetails>, PersistenceError>;

    /// Atomically creates the device, its initial shadow, and optional
    /// certificate, then returns the committed joined record.
    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceRecord,
        certificate: Option<NewDeviceCertificateRecord>,
    ) -> Result<DeviceDetails, PersistenceError>;

    async fn update(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: UpdateDeviceRecord,
    ) -> Result<Option<DeviceDetails>, PersistenceError>;

    async fn delete(&self, tenant: &TenantId, device_id: &str) -> Result<bool, PersistenceError>;

    async fn resolve_ids(
        &self,
        tenant: &TenantId,
        filter: DeviceFilter,
    ) -> Result<Vec<String>, PersistenceError>;

    async fn bulk_assign_fleet(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
        fleet_id: Option<i32>,
        updated_at: DateTime<Utc>,
    ) -> Result<usize, PersistenceError>;

    async fn bulk_delete(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
    ) -> Result<usize, PersistenceError>;
}
