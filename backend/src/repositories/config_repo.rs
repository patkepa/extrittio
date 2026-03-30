// Repository functions for device configs

use chrono::NaiveDateTime;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{DeviceConfig, NewDeviceConfig};
use crate::db::schema::device_configs;

pub fn find_config(
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<Option<DeviceConfig>, diesel::result::Error> {
    device_configs::table
        .find(device_id)
        .select(DeviceConfig::as_select())
        .first(conn)
        .optional()
}

pub fn upsert_config(
    conn: &mut PgConnection,
    device_id: &str,
    config: &str,
    now: NaiveDateTime,
) -> Result<DeviceConfig, diesel::result::Error> {
    diesel::insert_into(device_configs::table)
        .values(&NewDeviceConfig {
            device_id: device_id.to_string(),
            config: config.to_string(),
        })
        .on_conflict(device_configs::device_id)
        .do_update()
        .set((
            device_configs::config.eq(config),
            device_configs::updated_at.eq(now),
        ))
        .execute(conn)?;

    device_configs::table
        .find(device_id)
        .select(DeviceConfig::as_select())
        .first(conn)
}
