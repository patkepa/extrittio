use diesel::prelude::*;
use diesel::SqliteConnection;

use crate::db::models::{Device, DeviceType, Fleet, NewDevice, UpdateDevice};
use crate::db::schema::{device_types, devices, fleets};

pub fn list_devices(
    conn: &mut SqliteConnection,
    status_filter: Option<&str>,
    search_filter: Option<&str>,
    fleet_id_filter: Option<i32>,
) -> Result<Vec<(Device, DeviceType, Option<Fleet>)>, diesel::result::Error> {
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

    query
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .load(conn)
}

pub fn find_device_with_joins(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<(Device, DeviceType, Option<Fleet>), diesel::result::Error> {
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

pub fn find_device(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Device, diesel::result::Error> {
    devices::table
        .find(id)
        .select(Device::as_select())
        .first(conn)
}

pub fn device_exists(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<bool, diesel::result::Error> {
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

pub fn delete_device(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(devices::table.find(id)).execute(conn)?;
    Ok(rows > 0)
}
