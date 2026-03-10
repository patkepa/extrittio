// Repository functions for device configs

use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::SqliteConnection;

use crate::db::models::{DeviceConfig, NewDeviceConfig};
use crate::db::schema::device_configs;

pub fn find_config(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<Option<DeviceConfig>, diesel::result::Error> {
    device_configs::table
        .find(device_id)
        .select(DeviceConfig::as_select())
        .first(conn)
        .optional()
}

pub fn upsert_config(
    conn: &mut SqliteConnection,
    device_id: &str,
    config: &str,
    now: NaiveDateTime,
) -> Result<DeviceConfig, diesel::result::Error> {
    let existing = find_config(conn, device_id)?;

    if existing.is_some() {
        diesel::update(device_configs::table.find(device_id))
            .set((
                device_configs::config.eq(config),
                device_configs::updated_at.eq(now),
            ))
            .execute(conn)?;
    } else {
        diesel::insert_into(device_configs::table)
            .values(&NewDeviceConfig {
                device_id: device_id.to_string(),
                config: config.to_string(),
            })
            .execute(conn)?;
    }

    device_configs::table
        .find(device_id)
        .select(DeviceConfig::as_select())
        .first(conn)
}
