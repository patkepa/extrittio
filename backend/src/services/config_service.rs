use chrono::Utc;
use diesel::PgConnection;
use serde_json::{Map, Value};

use crate::db::models::DeviceConfig;
use crate::error::AppError;
use crate::repositories::{config_repo, device_repo};

/// Get the current configuration for a device, or None if not set.
/// Returns 404 if the device does not exist.
pub fn get_config(
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<Option<DeviceConfig>, AppError> {
    if !device_repo::device_exists(conn, device_id)? {
        return Err(AppError::NotFound(format!("Device '{device_id}' not found")));
    }
    Ok(config_repo::find_config(conn, device_id)?)
}

/// Merge a JSON patch into the device's configuration.
/// Merge semantics: null values remove keys, all other values upsert.
/// Returns 404 if the device does not exist.
pub fn merge_and_update(
    conn: &mut PgConnection,
    device_id: &str,
    patch: &Map<String, Value>,
) -> Result<DeviceConfig, AppError> {
    if !device_repo::device_exists(conn, device_id)? {
        return Err(AppError::NotFound(format!("Device '{device_id}' not found")));
    }

    let existing = config_repo::find_config(conn, device_id)?;
    let current: Value = existing
        .as_ref()
        .map_or(Value::Object(Map::default()), |c| {
            serde_json::from_str(&c.config).unwrap_or(Value::Object(Map::default()))
        });

    let mut obj = current.as_object().cloned().unwrap_or_default();
    for (key, val) in patch {
        if val.is_null() {
            obj.remove(key);
        } else {
            obj.insert(key.clone(), val.clone());
        }
    }

    let merged = Value::Object(obj);
    let now = Utc::now().naive_utc();
    let config_str = serde_json::to_string(&merged)?;

    Ok(config_repo::upsert_config(conn, device_id, &config_str, now)?)
}
