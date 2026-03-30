use diesel::Connection;
use diesel::PgConnection;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::warn;

use crate::db::models::{DeviceShadow, UpdateShadow};
use crate::error::AppError;
use crate::repositories::{firmware_repo, shadow_repo};
use extrittio_common::shadow::{compute_delta as compute_shadow_delta, merge_json};
use crate::state::{DbPool, ZenohMetrics, run_db};

/// DB-only part of update_desired. Returns the new delta and version so the
/// caller can publish via Zenoh after the transaction commits.
///
/// This function does NOT open its own transaction — the caller is responsible
/// for wrapping the call in a transaction when atomicity with other writes is
/// needed (e.g. `trigger_ota`). When called via `update_desired` the outer
/// `run_db` closure provides the transactional boundary.
pub fn update_desired_db(
    conn: &mut PgConnection,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<(Value, i32), AppError> {
    let shadow = shadow_repo::find_shadow(conn, device_id)?;

    let current_desired = if shadow.desired.is_object() {
        shadow.desired
    } else {
        Value::Object(serde_json::Map::default())
    };
    let current_reported = if shadow.reported.is_object() {
        shadow.reported
    } else {
        Value::Object(serde_json::Map::default())
    };

    let new_desired = merge_json(current_desired, patch);
    let new_delta = compute_shadow_delta(&new_desired, &current_reported);
    let new_version = shadow.version + 1;
    let now = chrono::Utc::now().naive_utc();

    let changeset = UpdateShadow {
        desired: Some(new_desired),
        delta: Some(new_delta.clone()),
        version: Some(new_version),
        updated_at: Some(now),
        ..Default::default()
    };

    shadow_repo::update_shadow(conn, device_id, &changeset)?;

    Ok((new_delta, new_version))
}

/// Merge a JSON patch into the desired state, recompute delta, persist, and publish.
pub async fn update_desired(
    pool: &DbPool,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
    zenoh_metrics: &ZenohMetrics,
) -> Result<(), AppError> {
    let d_id = device_id.to_string();
    let p = patch.clone();

    let (delta, version) = run_db(pool, move |conn| {
        conn.transaction(|conn| update_desired_db(conn, &d_id, &p))
    }).await?;

    publish_delta_if_nonempty(zenoh_session, device_id, &delta, version, zenoh_metrics).await;
    Ok(())
}

/// Merge a JSON patch into the reported state, recompute delta, persist.
pub fn update_reported(
    conn: &mut PgConnection,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<(), AppError> {
    conn.transaction(|conn| {
        let shadow = shadow_repo::find_shadow(conn, device_id)?;

        let current_desired = if shadow.desired.is_object() {
            shadow.desired
        } else {
            Value::Object(serde_json::Map::default())
        };
        let current_reported = if shadow.reported.is_object() {
            shadow.reported
        } else {
            Value::Object(serde_json::Map::default())
        };

        let new_reported = merge_json(current_reported, patch);
        let new_delta = compute_shadow_delta(&current_desired, &new_reported);
        let now = chrono::Utc::now().naive_utc();

        let changeset = UpdateShadow {
            reported: Some(new_reported),
            delta: Some(new_delta),
            version: Some(shadow.version + 1),
            updated_at: Some(now),
            ..Default::default()
        };

        shadow_repo::update_shadow(conn, device_id, &changeset)?;
        Ok(())
    })
}


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

/// Get the full shadow state for a device.
pub fn get_shadow(
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<DeviceShadow, AppError> {
    Ok(shadow_repo::find_shadow(conn, device_id)?)
}

/// Reset a device's shadow to empty state.
pub fn delete_shadow(
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<(), AppError> {
    let shadow = shadow_repo::find_shadow(conn, device_id)?;
    let now = chrono::Utc::now().naive_utc();
    let empty = Value::Object(serde_json::Map::default());
    let changeset = UpdateShadow {
        desired: Some(empty.clone()),
        reported: Some(empty.clone()),
        delta: Some(empty),
        version: Some(shadow.version + 1),
        updated_at: Some(now),
    };
    shadow_repo::update_shadow(conn, device_id, &changeset)?;
    Ok(())
}

/// Process OTA status from a shadow report's reported state.
pub fn process_ota_from_report(
    conn: &mut PgConnection,
    device_id: &str,
    reported: &serde_json::Value,
) -> Result<(), AppError> {
    use extrittio_common::ota::{fields as ota_fields, status as ota_status_consts};

    let ota_obj = match reported.get(ota_fields::SHADOW_KEY) {
        Some(serde_json::Value::Object(o)) => o,
        _ => return Ok(()),
    };

    let status_raw = match ota_obj.get(ota_fields::STATUS) {
        Some(serde_json::Value::String(s)) => s.as_str(),
        _ => return Ok(()),
    };
    let status = status_raw.to_lowercase();

    let fw_id = ota_obj
        .get(ota_fields::FIRMWARE_UPDATE_ID)
        .and_then(|v| v.as_i64())
        .and_then(|id| i32::try_from(id).ok());

    let deployment_id = firmware_repo::find_active_ota_deployment(conn, device_id, fw_id)?;

    if let Some(dep_id) = deployment_id {
        let error_msg = ota_obj
            .get(ota_fields::ERROR)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let completed_at = if ota_status_consts::is_terminal(&status) {
            Some(chrono::Utc::now().naive_utc())
        } else {
            None
        };

        firmware_repo::update_ota_deployment_status(
            conn, dep_id, &status, error_msg.as_deref(), completed_at,
        )?;
    }

    Ok(())
}
