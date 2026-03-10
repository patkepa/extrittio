// Repository functions for telemetry

use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::SqliteConnection;

use crate::db::models::{NewTelemetryRecord, TelemetryRecord};
use crate::db::schema::telemetry;

pub fn list_telemetry(
    conn: &mut SqliteConnection,
    device_id: &str,
    since: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRecord>, diesel::result::Error> {
    let mut query = telemetry::table
        .filter(telemetry::device_id.eq(device_id))
        .into_boxed();

    if let Some(since_dt) = since {
        query = query.filter(telemetry::received_at.gt(since_dt));
    }

    query
        .order(telemetry::received_at.desc())
        .limit(limit)
        .select(TelemetryRecord::as_select())
        .load(conn)
}

pub fn insert_telemetry(
    conn: &mut SqliteConnection,
    record: &NewTelemetryRecord,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(telemetry::table)
        .values(record)
        .execute(conn)?;
    Ok(())
}
