use diesel::Connection;
use diesel::SqliteConnection;
use serde_json::Value;
use std::sync::Arc;
use tracing::warn;

use crate::db::models::UpdateShadow;
use crate::error::AppError;
use crate::repositories::shadow_repo;
use extrittio_common::shadow::{compute_delta as compute_shadow_delta, merge_json};
use crate::state::{DbPool, run_db};

/// Transactional DB-only part of update_desired. Returns the new delta and
/// version so the caller can publish via Zenoh after the transaction commits.
pub fn update_desired_db(
    conn: &mut SqliteConnection,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<(Value, i32), AppError> {
    conn.transaction(|conn| {
        let shadow = shadow_repo::find_shadow(conn, device_id)?;

        let current_desired: Value = serde_json::from_str(&shadow.desired)
            .unwrap_or(Value::Object(serde_json::Map::default()));
        let current_reported: Value = serde_json::from_str(&shadow.reported)
            .unwrap_or(Value::Object(serde_json::Map::default()));

        let new_desired = merge_json(&current_desired, patch);
        let new_delta = compute_shadow_delta(&new_desired, &current_reported);
        let new_version = shadow.version + 1;
        let now = chrono::Utc::now().naive_utc();

        let changeset = UpdateShadow {
            desired: Some(serde_json::to_string(&new_desired)?),
            delta: Some(serde_json::to_string(&new_delta)?),
            version: Some(new_version),
            updated_at: Some(now),
            ..Default::default()
        };

        shadow_repo::update_shadow(conn, device_id, &changeset)?;

        Ok((new_delta, new_version))
    })
}

/// Merge a JSON patch into the desired state, recompute delta, persist, and publish.
pub async fn update_desired(
    pool: &DbPool,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<(), AppError> {
    let d_id = device_id.to_string();
    let p = patch.clone();

    let (delta, version) = run_db(pool, move |conn| update_desired_db(conn, &d_id, &p)).await?;

    publish_delta_if_nonempty(zenoh_session, device_id, &delta, version).await;
    Ok(())
}

/// Merge a JSON patch into the reported state, recompute delta, persist.
pub fn update_reported(
    conn: &mut SqliteConnection,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<(), AppError> {
    conn.transaction(|conn| {
        let shadow = shadow_repo::find_shadow(conn, device_id)?;

        let current_desired: Value = serde_json::from_str(&shadow.desired)
            .unwrap_or(Value::Object(serde_json::Map::default()));
        let current_reported: Value = serde_json::from_str(&shadow.reported)
            .unwrap_or(Value::Object(serde_json::Map::default()));

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
    })
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
        let delta_msg = extrittio_common::extrittio::ShadowDelta {
            device_id: device_id.to_string(),
            delta_json,
            version: i64::from(version),
        };
        let payload = prost::Message::encode_to_vec(&delta_msg);
        let topic = extrittio_common::topics::shadow_delta(device_id);
        if let Err(e) = session.put(&topic, payload).await {
            warn!("Failed to publish shadow delta to device {device_id}: {e}");
        }
    }
}
