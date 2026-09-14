use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::warn;

use crate::domains::firmware::port::FirmwareRepository;
use crate::domains::firmware::types::OtaStatusUpdate;
use crate::error::AppError;
use crate::state::ZenohMetrics;
use crate::tenancy::DeviceIdentity;

/// Publish a ShadowDelta via Zenoh if the delta is non-empty.
pub async fn publish_delta_if_nonempty(
    session: &Arc<zenoh::Session>,
    device_id: &str,
    delta: &Value,
    version: i32,
    zenoh_metrics: &ZenohMetrics,
) {
    if delta.as_object().is_some_and(|obj| !obj.is_empty()) {
        let delta_json = match serde_json::to_string(delta) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to serialize shadow delta for {device_id}: {e}");
                return;
            }
        };
        let delta_msg = extrittio_common::extrittio::ShadowDelta {
            device_id: device_id.to_string(),
            delta_json,
            version: i64::from(version),
        };
        let payload = prost::Message::encode_to_vec(&delta_msg);
        let topic = extrittio_common::topics::shadow_delta(device_id);
        if let Err(e) = session.put(&topic, payload).await {
            warn!("Failed to publish shadow delta to device {device_id}: {e}");
        } else {
            zenoh_metrics.messages_out.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub async fn process_ota_from_report_with_repository(
    repository: &dyn FirmwareRepository,
    identity: &DeviceIdentity,
    reported: &serde_json::Value,
) -> Result<(), AppError> {
    use extrittio_common::ota::{fields as ota_fields, status as ota_status_consts};

    let Some(serde_json::Value::Object(ota)) = reported.get(ota_fields::SHADOW_KEY) else {
        return Ok(());
    };
    let Some(status_raw) = ota
        .get(ota_fields::STATUS)
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(());
    };
    let status = status_raw.to_lowercase();
    let Some(deployment_id) = ota
        .get(ota_fields::DEPLOYMENT_ID)
        .and_then(serde_json::Value::as_i64)
        .and_then(|id| i32::try_from(id).ok())
        .filter(|id| *id > 0)
    else {
        return Ok(());
    };
    let firmware_update_id = ota
        .get(ota_fields::FIRMWARE_UPDATE_ID)
        .and_then(serde_json::Value::as_i64)
        .and_then(|id| i32::try_from(id).ok());
    let error_message = ota
        .get(ota_fields::ERROR)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let completed_at =
        ota_status_consts::is_terminal(&status).then(|| chrono::Utc::now().naive_utc());
    repository
        .apply_ota_status(
            identity,
            OtaStatusUpdate {
                deployment_id,
                firmware_update_id,
                status,
                error_message,
                completed_at,
            },
        )
        .await?;
    Ok(())
}
