use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domains::identity::certificate_types::NewDeviceCertificateRecord;
use crate::persistence::PersistenceError;
use crate::tenancy::DeviceIdentity;
use crate::tenancy::TenantId;

use super::types::{
    CreateDeviceRecord, DeviceContractRecord, DeviceDetails, DeviceFilter, DeviceIngressContext,
    DeviceList, DeviceListQuery, DeviceWriteOutcome, HeartbeatWrite, OfflineTransition,
    OfflineWriteOutcome, UpdateDeviceRecord,
};

#[async_trait]
pub trait DeviceRepository: Send + Sync {
    /// Resolves the globally unique protocol device ID to its tenant-qualified
    /// identity before any tenant-owned ingress operation runs.
    async fn resolve_identity(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceIdentity>, PersistenceError>;

    async fn ingress_context(
        &self,
        identity: &DeviceIdentity,
    ) -> Result<Option<DeviceIngressContext>, PersistenceError>;

    /// Updates heartbeat state and enqueues status-rule actions in one commit.
    /// `applied=false` requests a caller retry after concurrent state change.
    async fn apply_heartbeat(
        &self,
        identity: &DeviceIdentity,
        write: HeartbeatWrite,
    ) -> Result<DeviceWriteOutcome, PersistenceError>;

    async fn offline_candidates(
        &self,
        cutoff: chrono::NaiveDateTime,
    ) -> Result<Vec<DeviceIngressContext>, PersistenceError>;

    /// Rechecks cutoff and expected status, then commits offline transitions,
    /// logs, and rule actions atomically.
    async fn apply_offline_transitions(
        &self,
        cutoff: chrono::NaiveDateTime,
        transitions: Vec<OfflineTransition>,
    ) -> Result<OfflineWriteOutcome, PersistenceError>;

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

    /// Returns the contract currently assigned to the device. Until the device
    /// acknowledges a replacement, the desired contract is authoritative.
    async fn assigned_contract(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceContractRecord>, PersistenceError>;

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
