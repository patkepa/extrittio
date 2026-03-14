use diesel::SqliteConnection;
use diesel::prelude::*;

use crate::db::models::{Device, DeviceType, Fleet, NewDevice, UpdateDevice};
use crate::db::schema::{device_types, devices, fleets};

pub type DeviceWithJoins = (Device, DeviceType, Option<Fleet>);

pub fn list_devices(
    conn: &mut SqliteConnection,
    status_filter: Option<&str>,
    search_filter: Option<&str>,
    fleet_id_filter: Option<i32>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DeviceWithJoins>, i64), diesel::result::Error> {
    // Count query
    let mut count_query = devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .into_boxed();

    if let Some(status) = status_filter {
        count_query = count_query.filter(devices::status.eq(status));
    }
    if let Some(search) = search_filter {
        let pattern = format!("%{search}%");
        count_query = count_query.filter(
            devices::name
                .like(pattern.clone())
                .or(device_types::name.like(pattern.clone()))
                .or(devices::location.like(pattern)),
        );
    }
    if let Some(fleet_id) = fleet_id_filter {
        count_query = count_query.filter(devices::fleet_id.eq(fleet_id));
    }

    let total: i64 = count_query.count().get_result(conn)?;

    // Data query
    let mut query = devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .into_boxed();

    if let Some(status) = status_filter {
        query = query.filter(devices::status.eq(status));
    }
    if let Some(search) = search_filter {
        let pattern = format!("%{search}%");
        query = query.filter(
            devices::name
                .like(pattern.clone())
                .or(device_types::name.like(pattern.clone()))
                .or(devices::location.like(pattern)),
        );
    }
    if let Some(fleet_id) = fleet_id_filter {
        query = query.filter(devices::fleet_id.eq(fleet_id));
    }

    let results = query
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .limit(limit)
        .offset(offset)
        .load(conn)?;

    Ok((results, total))
}

pub fn find_device_with_joins(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<DeviceWithJoins, diesel::result::Error> {
    devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .filter(devices::id.eq(id))
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .first(conn)
}

pub fn find_device(conn: &mut SqliteConnection, id: &str) -> Result<Device, diesel::result::Error> {
    devices::table
        .find(id)
        .select(Device::as_select())
        .first(conn)
}

pub fn device_exists(conn: &mut SqliteConnection, id: &str) -> Result<bool, diesel::result::Error> {
    devices::table
        .find(id)
        .select(devices::id)
        .first::<String>(conn)
        .optional()
        .map(|opt| opt.is_some())
}

pub fn insert_device(
    conn: &mut SqliteConnection,
    new_device: &NewDevice,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(devices::table)
        .values(new_device)
        .execute(conn)?;
    Ok(())
}

pub fn update_device(
    conn: &mut SqliteConnection,
    id: &str,
    changeset: &UpdateDevice,
) -> Result<(), diesel::result::Error> {
    diesel::update(devices::table.find(id))
        .set(changeset)
        .execute(conn)?;
    Ok(())
}

pub fn delete_device(conn: &mut SqliteConnection, id: &str) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(devices::table.find(id)).execute(conn)?;
    Ok(rows > 0)
}

/// Bulk-update devices that haven't been seen since `cutoff` to "offline".
pub fn mark_devices_offline(
    conn: &mut SqliteConnection,
    cutoff: chrono::NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::update(
        devices::table
            .filter(devices::status.ne("offline"))
            .filter(devices::last_seen.lt(cutoff)),
    )
    .set(devices::status.eq("offline"))
    .execute(conn)
}

/// Return device IDs that are about to go offline (non-offline, last_seen < cutoff).
pub fn find_devices_going_offline(
    conn: &mut SqliteConnection,
    cutoff: chrono::NaiveDateTime,
) -> Result<Vec<String>, diesel::result::Error> {
    devices::table
        .filter(devices::status.ne("offline"))
        .filter(devices::last_seen.lt(cutoff))
        .select(devices::id)
        .load(conn)
}

/// Resolve device IDs from optional filters (for bulk operations).
pub fn resolve_device_ids(
    conn: &mut SqliteConnection,
    status_filter: Option<&str>,
    search_filter: Option<&str>,
    fleet_id_filter: Option<i32>,
) -> Result<Vec<String>, diesel::result::Error> {
    let mut query = devices::table
        .inner_join(device_types::table)
        .into_boxed();

    if let Some(status) = status_filter {
        query = query.filter(devices::status.eq(status));
    }
    if let Some(search) = search_filter {
        let pattern = format!("%{search}%");
        query = query.filter(
            devices::name
                .like(pattern.clone())
                .or(device_types::name.like(pattern.clone()))
                .or(devices::location.like(pattern)),
        );
    }
    if let Some(fleet_id) = fleet_id_filter {
        query = query.filter(devices::fleet_id.eq(fleet_id));
    }

    query.select(devices::id).load(conn)
}

/// Bulk-update fleet assignment for a list of device IDs.
pub fn bulk_update_fleet(
    conn: &mut SqliteConnection,
    ids: &[String],
    fleet_id: Option<i32>,
    now: chrono::NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::update(devices::table.filter(devices::id.eq_any(ids)))
        .set((
            devices::fleet_id.eq(fleet_id),
            devices::updated_at.eq(now),
        ))
        .execute(conn)
}

/// Bulk-delete devices by IDs.
pub fn bulk_delete_devices(
    conn: &mut SqliteConnection,
    ids: &[String],
) -> Result<usize, diesel::result::Error> {
    diesel::delete(devices::table.filter(devices::id.eq_any(ids))).execute(conn)
}
