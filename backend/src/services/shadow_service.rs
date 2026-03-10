use diesel::SqliteConnection;
use serde_json::Value;
use std::sync::Arc;
use tracing::warn;

use crate::db::models::UpdateShadow;
use crate::error::AppError;
use crate::repositories::shadow_repo;
use crate::shadow_utils::compute_shadow_delta;

/// Merge a JSON patch into the desired state, recompute delta, persist, and publish.
pub async fn update_desired(
    conn: &mut SqliteConnection,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<(), AppError> {
    let shadow = shadow_repo::find_shadow(conn, device_id)?;

    let current_desired: Value =
        serde_json::from_str(&shadow.desired).unwrap_or(Value::Object(Default::default()));
    let current_reported: Value =
        serde_json::from_str(&shadow.reported).unwrap_or(Value::Object(Default::default()));

    let new_desired = merge_json(&current_desired, patch);
    let new_delta = compute_shadow_delta(&new_desired, &current_reported);
    let now = chrono::Utc::now().naive_utc();

    let changeset = UpdateShadow {
        desired: Some(serde_json::to_string(&new_desired)?),
        delta: Some(serde_json::to_string(&new_delta)?),
        version: Some(shadow.version + 1),
        updated_at: Some(now),
        ..Default::default()
    };

    shadow_repo::update_shadow(conn, device_id, &changeset)?;

    // Note: We hold `conn` across this `.await` point. This works because
    // r2d2::PooledConnection<SqliteConnection> is Send.
    publish_delta_if_nonempty(zenoh_session, device_id, &new_delta, shadow.version + 1).await;
    Ok(())
}

/// Merge a JSON patch into the reported state, recompute delta, persist.
pub fn update_reported(
    conn: &mut SqliteConnection,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<(), AppError> {
    let shadow = shadow_repo::find_shadow(conn, device_id)?;

    let current_desired: Value =
        serde_json::from_str(&shadow.desired).unwrap_or(Value::Object(Default::default()));
    let current_reported: Value =
        serde_json::from_str(&shadow.reported).unwrap_or(Value::Object(Default::default()));

    let new_reported = merge_json(&current_reported, patch);
    let new_delta = compute_shadow_delta(&current_desired, &new_reported);
    let now = chrono::Utc::now().naive_utc();

    let changeset = UpdateShadow {
        reported: Some(serde_json::to_string(&new_reported)?),
        delta: Some(serde_json::to_string(&new_delta)?),
        version: Some(shadow.version + 1),
        updated_at: Some(now),
        ..Default::default()
    };

    shadow_repo::update_shadow(conn, device_id, &changeset)?;
    Ok(())
}

/// Merge a JSON patch into an existing JSON object.
/// Keys with null values are removed.
pub fn merge_json(existing: &Value, patch: &serde_json::Map<String, Value>) -> Value {
    let mut obj = existing.as_object().cloned().unwrap_or_default();
    for (key, val) in patch {
        if val.is_null() {
            obj.remove(key);
        } else {
            obj.insert(key.clone(), val.clone());
        }
    }
    Value::Object(obj)
}

/// Publish a ShadowDelta via Zenoh if the delta is non-empty.
pub async fn publish_delta_if_nonempty(
    session: &Arc<zenoh::Session>,
    device_id: &str,
    delta: &Value,
    version: i32,
) {
    if delta.as_object().is_some_and(|obj| !obj.is_empty()) {
        let delta_json = match serde_json::to_string(delta) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to serialize shadow delta for {device_id}: {e}");
                return;
            }
        };
        let delta_msg = extrittio_proto::extrittio::ShadowDelta {
            device_id: device_id.to_string(),
            delta_json,
            version: i64::from(version),
        };
        let payload = prost::Message::encode_to_vec(&delta_msg);
        let topic = format!("extrittio/devices/{device_id}/shadow/delta");
        if let Err(e) = session.put(&topic, payload).await {
            warn!("Failed to publish shadow delta to device {device_id}: {e}");
        }
    }
}
