use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OtaStatusUpdate {
    pub firmware_update_id: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
    pub completed_at: Option<NaiveDateTime>,
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

#[derive(Debug, Clone)]
pub struct CiIngestParams {
    pub device_type_name: String,
    pub version: String,
    pub artifact_url: String,
    pub sha256: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<NaiveDateTime>,
    pub description: Option<String>,
    pub changelog: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CiIngestOutcome {
    Unauthorized,
    DeviceTypeNotFound,
    Forbidden {
        scoped_device_type_id: i32,
    },
    Created {
        firmware_id: i32,
        version: String,
        device_type_name: String,
    },
}
