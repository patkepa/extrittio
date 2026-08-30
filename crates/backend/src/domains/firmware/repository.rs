use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::types::{
    CiIngestOutcome, CiIngestParams, FirmwareBlobRecord, FirmwarePage, FirmwareRecord,
    GlobalOtaDeploymentPage, LegacyFirmwareBlob, NewFirmwareBlobRecord, NewFirmwareRecord,
    OtaDeploymentPage, OtaStatusUpdate, TriggerOtaOutcome,
};

#[async_trait]
pub trait FirmwareRepository: Send + Sync {
    async fn ingest_ci(
        &self,
        key_hash: &str,
        params: CiIngestParams,
    ) -> Result<CiIngestOutcome, PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        device_type_id: Option<i32>,
        blueprint_revision_id: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<FirmwarePage, PersistenceError>;

    async fn list_all_deployments(
        &self,
        tenant: &TenantId,
        status: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<GlobalOtaDeploymentPage, PersistenceError>;

    async fn create(
        &self,
        tenant: &TenantId,
        record: NewFirmwareRecord,
        blob: Option<NewFirmwareBlobRecord>,
    ) -> Result<Option<FirmwareRecord>, PersistenceError>;

    async fn next_version(
        &self,
        tenant: &TenantId,
        device_type_id: i32,
    ) -> Result<String, PersistenceError>;

    async fn next_blueprint_version(
        &self,
        tenant: &TenantId,
        blueprint_revision_id: &str,
    ) -> Result<String, PersistenceError>;

    async fn get_blob(
        &self,
        tenant: &TenantId,
        firmware_update_id: i32,
    ) -> Result<Option<FirmwareBlobRecord>, PersistenceError>;

    async fn delete(
        &self,
        tenant: &TenantId,
        firmware_update_id: i32,
    ) -> Result<Option<Option<FirmwareBlobRecord>>, PersistenceError>;

    async fn apply_ota_status(
        &self,
        identity: &DeviceIdentity,
        update: OtaStatusUpdate,
    ) -> Result<bool, PersistenceError>;

    async fn list_device_deployments(
        &self,
        tenant: &TenantId,
        device_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Option<OtaDeploymentPage>, PersistenceError>;

    async fn trigger_ota(
        &self,
        tenant: &TenantId,
        device_id: &str,
        firmware_update_id: i32,
        public_url: &str,
    ) -> Result<TriggerOtaOutcome, PersistenceError>;

    async fn next_legacy_blob(&self) -> Result<Option<LegacyFirmwareBlob>, PersistenceError>;

    async fn mark_blob_migrated(
        &self,
        tenant_id: &str,
        firmware_update_id: i32,
        storage_backend: &str,
        storage_key: &str,
    ) -> Result<bool, PersistenceError>;
}
