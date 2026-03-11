// Repository functions for device shadows

use diesel::SqliteConnection;
use diesel::prelude::*;

use crate::db::models::{DeviceShadow, NewDeviceShadow, UpdateShadow};
use crate::db::schema::device_shadows;

pub fn find_shadow(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<DeviceShadow, diesel::result::Error> {
    device_shadows::table
        .find(device_id)
        .select(DeviceShadow::as_select())
        .first(conn)
}

pub fn find_shadow_optional(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<Option<DeviceShadow>, diesel::result::Error> {
    device_shadows::table
        .find(device_id)
        .select(DeviceShadow::as_select())
        .first(conn)
        .optional()
}

pub fn insert_shadow(
    conn: &mut SqliteConnection,
    new_shadow: &NewDeviceShadow,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(device_shadows::table)
        .values(new_shadow)
        .execute(conn)?;
    Ok(())
}

pub fn update_shadow(
    conn: &mut SqliteConnection,
    device_id: &str,
    changeset: &UpdateShadow,
) -> Result<usize, diesel::result::Error> {
    diesel::update(device_shadows::table.find(device_id))
        .set(changeset)
        .execute(conn)
}
