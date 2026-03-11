use extrittio_common::ota::fields;
use serde_json::Value;

/// Parsed OTA payload extracted from a shadow delta.
pub struct OtaPayload {
    pub firmware_version: String,
    pub firmware_url: String,
    pub firmware_update_id: Option<i64>,
    pub sha256: Option<String>,
}

impl OtaPayload {
    /// Parse an OTA payload from a JSON value.
    /// Returns `None` if required fields (`firmware_version`, `firmware_url`) are missing.
    pub fn from_json(value: &Value) -> Option<Self> {
        Some(Self {
            firmware_version: value.get(fields::FIRMWARE_VERSION)?.as_str()?.to_string(),
            firmware_url: value.get(fields::FIRMWARE_URL)?.as_str()?.to_string(),
            firmware_update_id: value.get(fields::FIRMWARE_UPDATE_ID).and_then(Value::as_i64),
            sha256: value
                .get(fields::SHA256)
                .and_then(Value::as_str)
                .map(String::from),
        })
    }
}

/// Build a JSON object for reporting OTA status in the device shadow.
pub fn build_status_json(
    status: &str,
    fw_version: &str,
    fw_update_id: Option<i64>,
    error: Option<&str>,
) -> Value {
    let mut obj = serde_json::json!({
        fields::STATUS: status,
        fields::FIRMWARE_VERSION: fw_version,
    });
    if let Some(id) = fw_update_id {
        obj[fields::FIRMWARE_UPDATE_ID] = serde_json::json!(id);
    }
    if let Some(err) = error {
        obj[fields::ERROR] = serde_json::json!(err);
    }
    obj
}
