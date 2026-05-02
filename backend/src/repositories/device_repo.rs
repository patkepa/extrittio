use diesel::PgConnection;
use diesel::PgTextExpressionMethods;
use diesel::prelude::*;

use crate::db::models::{Device, DeviceType, Fleet, NewDevice, UpdateDevice};
use crate::db::schema::{device_types, devices, fleets};

pub type DeviceWithJoins = (Device, DeviceType, Option<Fleet>);

type BoxedDeviceQuery<'a> = diesel::dsl::IntoBoxed<
    'a,
    diesel::dsl::LeftJoin<
        diesel::dsl::InnerJoin<devices::table, device_types::table>,
        fleets::table,
    >,
    diesel::pg::Pg,
>;

/// Build a filtered query for devices with joins. Shared by count and data queries.
fn filtered_device_query<'a>(
    tenant_id: &'a str,
    status_filter: Option<&'a str>,
    search_filter: Option<&'a str>,
    fleet_id_filter: Option<i32>,
) -> BoxedDeviceQuery<'a> {
    let mut query = devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .into_boxed();

    query = query.filter(devices::tenant_id.eq(tenant_id));

    if let Some(status) = status_filter {
        query = query.filter(devices::status.eq(status));
    }
    if let Some(search) = search_filter {
        let pattern = format!("%{search}%");
        query = query.filter(
            devices::name
                .ilike(pattern.clone())
                .or(device_types::name.ilike(pattern)),
        );
    }
    if let Some(fleet_id) = fleet_id_filter {
        query = query.filter(devices::fleet_id.eq(fleet_id));
    }

    query
}

pub fn list_devices(
    conn: &mut PgConnection,
    tenant_id: &str,
    status_filter: Option<&str>,
    search_filter: Option<&str>,
    fleet_id_filter: Option<i32>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DeviceWithJoins>, i64), diesel::result::Error> {
    let total: i64 =
        filtered_device_query(tenant_id, status_filter, search_filter, fleet_id_filter)
            .count()
            .get_result(conn)?;

    let results = filtered_device_query(tenant_id, status_filter, search_filter, fleet_id_filter)
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

/// Resolve device IDs matching the given filters (no pagination).
/// Joins device_types because the search filter searches device_types::name.
pub fn resolve_device_ids(
    conn: &mut PgConnection,
    tenant_id: &str,
    status_filter: Option<&str>,
    search_filter: Option<&str>,
    fleet_id_filter: Option<i32>,
) -> Result<Vec<String>, diesel::result::Error> {
    let mut query = devices::table.inner_join(device_types::table).into_boxed();
    query = query.filter(devices::tenant_id.eq(tenant_id));

    if let Some(status) = status_filter {
        query = query.filter(devices::status.eq(status));
    }
    if let Some(search) = search_filter {
        let pattern = format!("%{search}%");
        query = query.filter(
            devices::name
                .ilike(pattern.clone())
                .or(device_types::name.ilike(pattern)),
        );
    }
    if let Some(fleet_id) = fleet_id_filter {
        query = query.filter(devices::fleet_id.eq(fleet_id));
    }

    query.select(devices::id).load(conn)
}

/// Bulk-update fleet assignment for the given device IDs.
pub fn bulk_update_fleet(
    conn: &mut PgConnection,
    tenant_id: &str,
    ids: &[String],
    fleet_id: Option<i32>,
    now: chrono::NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::update(
        devices::table
            .filter(devices::tenant_id.eq(tenant_id))
            .filter(devices::id.eq_any(ids)),
    )
    .set((devices::fleet_id.eq(fleet_id), devices::updated_at.eq(now)))
    .execute(conn)
}

/// Bulk-delete devices by IDs. Returns the number of rows deleted.
pub fn bulk_delete_devices(
    conn: &mut PgConnection,
    tenant_id: &str,
    ids: &[String],
) -> Result<usize, diesel::result::Error> {
    diesel::delete(
        devices::table
            .filter(devices::tenant_id.eq(tenant_id))
            .filter(devices::id.eq_any(ids)),
    )
    .execute(conn)
}

pub fn find_device_with_joins(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: &str,
) -> Result<DeviceWithJoins, diesel::result::Error> {
    devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .filter(devices::tenant_id.eq(tenant_id))
        .filter(devices::id.eq(id))
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .first(conn)
}

pub fn find_device(conn: &mut PgConnection, id: &str) -> Result<Device, diesel::result::Error> {
    devices::table
        .find(id)
        .select(Device::as_select())
        .first(conn)
}

pub fn find_device_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: &str,
) -> Result<Device, diesel::result::Error> {
    devices::table
        .filter(devices::tenant_id.eq(tenant_id))
        .filter(devices::id.eq(id))
        .select(Device::as_select())
        .first(conn)
}

pub fn device_exists(conn: &mut PgConnection, id: &str) -> Result<bool, diesel::result::Error> {
    devices::table
        .find(id)
        .select(devices::id)
        .first::<String>(conn)
        .optional()
        .map(|opt| opt.is_some())
}

pub fn insert_device(
    conn: &mut PgConnection,
    new_device: &NewDevice,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(devices::table)
        .values(new_device)
        .execute(conn)?;
    Ok(())
}

pub fn update_device(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: &str,
    changeset: &UpdateDevice,
) -> Result<(), diesel::result::Error> {
    diesel::update(
        devices::table
            .filter(devices::tenant_id.eq(tenant_id))
            .filter(devices::id.eq(id)),
    )
    .set(changeset)
    .execute(conn)?;
    Ok(())
}

pub fn delete_device(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: &str,
) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(
        devices::table
            .filter(devices::tenant_id.eq(tenant_id))
            .filter(devices::id.eq(id)),
    )
    .execute(conn)?;
    Ok(rows > 0)
}

/// Find device IDs that will be marked offline (not already offline, last seen before cutoff).
pub fn find_devices_going_offline(
    conn: &mut PgConnection,
    cutoff: chrono::NaiveDateTime,
) -> Result<Vec<String>, diesel::result::Error> {
    devices::table
        .filter(devices::status.ne("offline"))
        .filter(devices::last_seen.lt(cutoff))
        .select(devices::id)
        .load(conn)
}

/// Bulk-update devices that haven't been seen since `cutoff` to "offline".
pub fn mark_devices_offline(
    conn: &mut PgConnection,
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
