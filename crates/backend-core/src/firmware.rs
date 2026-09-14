use crate::{DeviceIdentity, PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OtaStatusUpdate {
    pub deployment_id: i32,
    pub firmware_update_id: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
    pub completed_at: Option<NaiveDateTime>,
}

/// Only forward transitions are accepted; terminal deployments are immutable.
pub fn ota_transition_allowed(previous: &str, next: &str) -> bool {
    fn rank(status: &str) -> Option<u8> {
        Some(match status {
            "pending" => 0,
            "downloading" => 1,
            "verifying" => 2,
            "installing" => 3,
            "rebooting" => 4,
            "success" | "failed" => 5,
            _ => return None,
        })
    }
    !ota_status_is_terminal(previous)
        && matches!((rank(previous), rank(next)), (Some(a), Some(b)) if b >= a)
}

#[derive(Debug, Clone)]
pub struct LegacyFirmwareBlob {
    pub tenant_id: String,
    pub firmware_update_id: i32,
    pub filename: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct OtaDeploymentRecord {
    pub id: i32,
    pub device_id: String,
    pub firmware_update_id: i32,
    pub firmware_version: String,
    pub status: String,
    pub error_message: Option<String>,
    pub initiated_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct OtaDeploymentPage {
    pub records: Vec<OtaDeploymentRecord>,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub enum TriggerOtaOutcome {
    DeviceNotFound,
    FirmwareNotFound,
    Incompatible,
    InvalidArtifact,
    Ready { delta: Value, version: i32 },
}

pub fn valid_ota_artifact(version: &str, hash: Option<&str>, url: &str) -> bool {
    !version.is_empty()
        && version.len() <= 63
        && url.len() <= 1023
        && (url.starts_with("https://") || url.starts_with("http://"))
        && hash.is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()))
}

#[derive(Debug, Clone)]
pub struct FirmwareRecord {
    pub id: i32,
    pub device_type_id: i32,
    pub device_type_name: String,
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub file_size: Option<i32>,
    pub filename: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<NaiveDateTime>,
    pub changelog: Option<String>,
    pub source: String,
    pub blueprint_revision_id: Option<String>,
    pub compatibility: Value,
    pub update_strategy: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FirmwarePage {
    pub records: Vec<FirmwareRecord>,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct NewFirmwareRecord {
    pub device_type_id: i32,
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub description: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<NaiveDateTime>,
    pub changelog: Option<String>,
    pub source: Option<String>,
    pub blueprint_revision_id: Option<String>,
    pub compatibility: Value,
    pub update_strategy: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewFirmwareBlobRecord {
    pub size: i32,
    pub filename: String,
    pub storage_key: String,
    pub storage_backend: String,
}

#[derive(Debug, Clone)]
pub struct FirmwareBlobRecord {
    pub data: Option<Vec<u8>>,
    pub size: i32,
    pub filename: String,
    pub storage_key: Option<String>,
    pub storage_backend: String,
}

#[derive(Debug, Clone)]
pub struct GlobalOtaDeploymentRecord {
    pub id: i32,
    pub device_id: String,
    pub device_name: String,
    pub device_status: String,
    pub current_firmware: String,
    pub device_type_id: i32,
    pub device_type_name: String,
    pub fleet_id: Option<i32>,
    pub fleet_name: Option<String>,
    pub firmware_update_id: i32,
    pub firmware_version: String,
    pub status: String,
    pub error_message: Option<String>,
    pub initiated_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct GlobalOtaDeploymentPage {
    pub records: Vec<GlobalOtaDeploymentRecord>,
    pub total: i64,
}

pub fn ota_status_is_terminal(status: &str) -> bool {
    status.eq_ignore_ascii_case("success") || status.eq_ignore_ascii_case("failed")
}

#[async_trait]
pub trait FirmwareRepository: Send + Sync {
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

/// Host capability for firmware objects; database orchestration belongs to core.
#[async_trait]
pub trait FirmwareObjectStorage: Send + Sync {
    fn backend(&self) -> &str;
    fn allocate_key(&self, tenant: &TenantId, filename: &str) -> String;
    fn legacy_key(&self, tenant: &str, firmware_update_id: i32, filename: &str) -> String;
    async fn get(&self, key: &str) -> Result<Vec<u8>, String>;
    async fn put(&self, key: &str, data: Vec<u8>) -> Result<(), String>;
    async fn delete(&self, key: &str) -> Result<(), String>;
}

#[derive(Debug)]
pub struct FirmwareDownload {
    pub data: Vec<u8>,
    pub filename: String,
    pub size: i32,
}

/// A grant whose signature, audience, and expiry have been verified by the host.
/// Construction validates the domain scope; it does not verify a raw token.
#[derive(Debug)]
pub struct VerifiedFirmwareDownload {
    tenant: TenantId,
    firmware_id: i32,
}
impl VerifiedFirmwareDownload {
    pub fn from_verified_claims(
        tenant: String,
        firmware_id: i32,
    ) -> Result<Self, crate::ApplicationError> {
        if firmware_id <= 0 {
            return Err(crate::ApplicationError::Unauthorized);
        }
        let tenant = TenantId::new(tenant).map_err(|_| crate::ApplicationError::Unauthorized)?;
        Ok(Self {
            tenant,
            firmware_id,
        })
    }
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }
    pub fn firmware_id(&self) -> i32 {
        self.firmware_id
    }
}

/// Signing inputs owned by core; the host selects the existing JWT encoding/key.
pub struct FirmwareDownloadGrant {
    pub tenant: TenantId,
    pub firmware_id: i32,
    pub audience: &'static str,
    pub expires_at: u64,
}
impl FirmwareDownloadGrant {
    pub fn new(
        tenant: TenantId,
        firmware_id: i32,
        clock: &dyn crate::Clock,
    ) -> Result<Self, crate::ApplicationError> {
        let expires_at = clock
            .now()
            .timestamp()
            .checked_add(7 * 24 * 3600)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| {
                crate::ApplicationError::Internal("Failed to authorize firmware download".into())
            })?;
        Ok(Self {
            tenant,
            firmware_id,
            audience: "extrittio:firmware-download",
            expires_at,
        })
    }
}
pub trait FirmwareDownloadSigner: Send + Sync {
    fn sign(&self, grant: &FirmwareDownloadGrant) -> Result<String, crate::ApplicationError>;
}

pub fn increment_firmware_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 3
        && let Ok(patch) = parts[2].parse::<u32>()
    {
        return format!("{}.{}.{}", parts[0], parts[1], patch + 1);
    }
    format!("{version}.1")
}

/// Validated OTA artifact policy shared by transaction-owning adapters.
pub struct PreparedOtaArtifact {
    version: String,
    url: String,
    sha256: String,
}
impl PreparedOtaArtifact {
    pub fn prepare(
        version: &str,
        hash: Option<&str>,
        stored_url: &str,
        grant_url: &str,
    ) -> Option<Self> {
        let url = if stored_url.starts_with("https://") {
            stored_url
        } else {
            grant_url
        };
        if !valid_ota_artifact(version, hash, url) {
            return None;
        }
        Some(Self {
            version: version.into(),
            url: url.into(),
            sha256: hash?.into(),
        })
    }
    pub fn desired_patch(
        self,
        firmware_id: i32,
        deployment_id: i64,
    ) -> serde_json::Map<String, Value> {
        let mut patch = serde_json::Map::new();
        patch.insert(
            "ota".into(),
            serde_json::json!({
                "firmware_version": self.version, "firmware_url": self.url,
                "firmware_update_id": firmware_id, "deployment_id": deployment_id,
                "sha256": self.sha256,
            }),
        );
        patch
    }
}
