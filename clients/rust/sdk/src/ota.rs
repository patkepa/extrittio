#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use extrittio_common::ota::fields;
use serde_json::Value;

/// Parsed OTA payload extracted from a shadow delta.
pub struct OtaPayload {
    pub deployment_id: i64,
    pub firmware_version: String,
    pub firmware_url: String,
    pub firmware_update_id: Option<i64>,
    pub sha256: Option<String>,
}

impl OtaPayload {
    /// Parse an OTA payload from a JSON value.
    /// Requires a version, HTTP(S) URL, checksum, firmware ID and deployment ID.
    pub fn from_json(value: &Value) -> Option<Self> {
        let hash = value.get(fields::SHA256)?.as_str()?;
        let version = value.get(fields::FIRMWARE_VERSION)?.as_str()?;
        let url = value.get(fields::FIRMWARE_URL)?.as_str()?;
        if hash.len() != 64
            || !hash.bytes().all(|c| c.is_ascii_hexdigit())
            || version.is_empty()
            || version.len() > 63
            || url.len() > 1023
            || !(url.starts_with("https://") || url.starts_with("http://"))
        {
            return None;
        }
        let deployment_id = value.get(fields::DEPLOYMENT_ID)?.as_i64()?;
        if deployment_id <= 0 || value.get(fields::FIRMWARE_UPDATE_ID)?.as_i64()? <= 0 {
            return None;
        }
        Some(Self {
            deployment_id,
            firmware_version: value.get(fields::FIRMWARE_VERSION)?.as_str()?.to_string(),
            firmware_url: value.get(fields::FIRMWARE_URL)?.as_str()?.to_string(),
            firmware_update_id: value
                .get(fields::FIRMWARE_UPDATE_ID)
                .and_then(Value::as_i64),
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
    deployment_id: i64,
    error: Option<&str>,
) -> Value {
    let mut obj = serde_json::json!({
        fields::STATUS: status,
        fields::FIRMWARE_VERSION: fw_version,
        fields::DEPLOYMENT_ID: deployment_id,
    });
    if let Some(id) = fw_update_id {
        obj[fields::FIRMWARE_UPDATE_ID] = serde_json::json!(id);
    }
    if let Some(err) = error {
        obj[fields::ERROR] = serde_json::json!(err);
    }
    obj
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifests_require_integrity_and_attempt_identity() {
        let manifest = serde_json::json!({"firmware_version":"2.0.0", "firmware_url":"https://example.com/fw",
            "firmware_update_id":42, "deployment_id":7, "sha256":"a".repeat(64)});
        assert_eq!(OtaPayload::from_json(&manifest).unwrap().deployment_id, 7);
        for key in [
            "sha256",
            "deployment_id",
            "firmware_update_id",
            "firmware_version",
            "firmware_url",
        ] {
            let mut missing = manifest.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(
                OtaPayload::from_json(&missing).is_none(),
                "accepted missing {key}"
            );
        }
        for (key, value) in [
            ("sha256", serde_json::json!("x".repeat(64))),
            ("deployment_id", serde_json::json!(0)),
            ("firmware_url", serde_json::json!("file:///tmp/image")),
        ] {
            let mut invalid = manifest.clone();
            invalid[key] = value;
            assert!(OtaPayload::from_json(&invalid).is_none());
        }
        let report = build_status_json("success", "2.0.0", Some(42), 7, None);
        assert_eq!(report["deployment_id"], 7);
    }
}
