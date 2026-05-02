// Repository functions for telemetry

use chrono::NaiveDateTime;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::{Array, Jsonb, Text};
use serde_json::Value as JsonValue;

use crate::db::models::{NewTelemetryRecord, TelemetryRecord};
use crate::db::schema::telemetry;

#[derive(QueryableByName)]
pub struct LatestTelemetryCustomJson {
    #[diesel(sql_type = Text)]
    pub device_id: String,
    #[diesel(sql_type = Jsonb)]
    pub custom_json: JsonValue,
}

pub fn list_telemetry(
    conn: &mut PgConnection,
    device_id: &str,
    since: Option<NaiveDateTime>,
    before: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRecord>, diesel::result::Error> {
    let mut query = telemetry::table
        .filter(telemetry::device_id.eq(device_id))
        .into_boxed();

    if let Some(since_dt) = since {
        query = query.filter(telemetry::received_at.gt(since_dt));
    }

    if let Some(before_dt) = before {
        query = query.filter(telemetry::received_at.lt(before_dt));
    }

    query
        .order(telemetry::received_at.desc())
        .limit(limit)
        .select(TelemetryRecord::as_select())
        .load(conn)
}

pub fn get_latest_location(
    conn: &mut PgConnection,
    dev_id: &str,
) -> QueryResult<Option<TelemetryRecord>> {
    use crate::db::schema::telemetry::dsl::*;
    telemetry
        .filter(device_id.eq(dev_id))
        .filter(latitude.is_not_null())
        .filter(longitude.is_not_null())
        .order(received_at.desc())
        .select(TelemetryRecord::as_select())
        .first(conn)
        .optional()
}

pub fn insert_telemetry(
    conn: &mut PgConnection,
    record: &NewTelemetryRecord,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(telemetry::table)
        .values(record)
        .execute(conn)?;
    Ok(())
}

pub fn latest_connection_sources_for_devices(
    conn: &mut PgConnection,
    device_ids: &[String],
) -> Result<Vec<LatestTelemetryCustomJson>, diesel::result::Error> {
    if device_ids.is_empty() {
        return Ok(Vec::new());
    }

    diesel::sql_query(
        r#"
        SELECT DISTINCT ON (device_id) device_id, custom_json
        FROM telemetry
        WHERE device_id = ANY($1)
          AND custom_json->>'kind' = 'network_analyzer_scan'
          AND custom_json ? 'snapshot_json'
        ORDER BY device_id, received_at DESC
        "#,
    )
    .bind::<Array<Text>, _>(device_ids)
    .load(conn)
}
