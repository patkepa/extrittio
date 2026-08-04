// Repository functions for device logs

use chrono::NaiveDateTime;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{DeviceLog, NewDeviceLog};
use crate::db::schema::device_logs;

pub fn list_logs(
    conn: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
    level: Option<&str>,
    since: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<DeviceLog>, diesel::result::Error> {
    let mut query = device_logs::table
        .filter(device_logs::tenant_id.eq(tenant_id))
        .filter(device_logs::device_id.eq(device_id))
        .into_boxed();

    if let Some(level) = level {
        query = query.filter(device_logs::level.eq(level));
    }

    if let Some(since_dt) = since {
        query = query.filter(device_logs::created_at.gt(since_dt));
    }

    query
        .order(device_logs::created_at.desc())
        .limit(limit)
        .select(DeviceLog::as_select())
        .load(conn)
}

pub fn insert_log(
    conn: &mut PgConnection,
    record: &NewDeviceLog,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(device_logs::table)
        .values(record)
        .execute(conn)?;
    Ok(())
}

pub fn delete_older_than(
    conn: &mut PgConnection,
    cutoff: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(device_logs::table.filter(device_logs::created_at.lt(cutoff))).execute(conn)
}
