use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LoginResponse {
    pub(crate) token: String,
    pub(crate) user: UserResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct UserResponse {
    pub(crate) id: i32,
    pub(crate) username: String,
    pub(crate) role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Paginated<T> {
    pub(crate) data: Vec<T>,
    pub(crate) total: i64,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DeviceResponse {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) device_type_id: i32,
    pub(crate) device_type_name: String,
    pub(crate) fleet_id: Option<i32>,
    pub(crate) fleet_name: Option<String>,
    pub(crate) status: String,
    pub(crate) last_seen: String,
    pub(crate) last_seen_at: Option<String>,
    pub(crate) firmware: String,
    pub(crate) uptime: String,
    pub(crate) uptime_seconds: i32,
    pub(crate) latest_latitude: Option<f64>,
    pub(crate) latest_longitude: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DeviceTypeResponse {
    pub(crate) id: i32,
    pub(crate) name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FleetResponse {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) device_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ApiKeyResponse {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) key_prefix: String,
    pub(crate) device_type_id: Option<i32>,
    pub(crate) device_type_name: Option<String>,
    pub(crate) created_at: String,
    pub(crate) last_used_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CreatedApiKeyResponse {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) key: String,
    pub(crate) key_prefix: String,
    pub(crate) device_type_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CertificateBundle {
    pub(crate) certificate_pem: String,
    pub(crate) private_key_pem: String,
    pub(crate) ca_pem: String,
    pub(crate) fingerprint: String,
    pub(crate) expires_at: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CaCertificate {
    pub(crate) fingerprint: String,
    pub(crate) certificate_pem: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CertificateStatus {
    pub(crate) fingerprint: String,
    pub(crate) expires_at: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct Esp32NvsFlashResult {
    pub(crate) port: String,
    pub(crate) chip: String,
    pub(crate) baud: u32,
    pub(crate) nvs_offset: String,
    pub(crate) nvs_size: String,
    pub(crate) csv_path: Option<PathBuf>,
    pub(crate) bin_path: Option<PathBuf>,
}
