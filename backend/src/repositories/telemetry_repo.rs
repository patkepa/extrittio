// Repository functions for telemetry

use chrono::NaiveDateTime;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::{Array, Jsonb, Text, Timestamptz};
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
    tenant_id: &str,
    device_id: &str,
    since: Option<NaiveDateTime>,
    before: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRecord>, diesel::result::Error> {
    let mut query = telemetry::table
        .filter(telemetry::tenant_id.eq(tenant_id))
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
    tenant_id_filter: &str,
    dev_id: &str,
) -> QueryResult<Option<TelemetryRecord>> {
    use crate::db::schema::telemetry::dsl::*;
    telemetry
        .filter(tenant_id.eq(tenant_id_filter))
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

pub fn upsert_hourly_rollups(
    conn: &mut PgConnection,
    since: NaiveDateTime,
    before: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::sql_query(
        r#"
        INSERT INTO telemetry_rollups_hourly (
            tenant_id,
            device_id,
            bucket_start,
            sample_count,
            avg_temperature,
            min_temperature,
            max_temperature,
            avg_humidity,
            min_humidity,
            max_humidity,
            avg_battery_level,
            min_battery_level,
            max_battery_level,
            updated_at
        )
        SELECT
            tenant_id,
            device_id,
            date_trunc('hour', received_at) AS bucket_start,
            count(*) AS sample_count,
            avg(temperature)::real AS avg_temperature,
            min(temperature)::real AS min_temperature,
            max(temperature)::real AS max_temperature,
            avg(humidity)::real AS avg_humidity,
            min(humidity)::real AS min_humidity,
            max(humidity)::real AS max_humidity,
            avg(battery_level)::real AS avg_battery_level,
            min(battery_level)::real AS min_battery_level,
            max(battery_level)::real AS max_battery_level,
            now() AS updated_at
        FROM telemetry
        WHERE received_at >= $1
          AND received_at < $2
        GROUP BY tenant_id, device_id, date_trunc('hour', received_at)
        ON CONFLICT (tenant_id, device_id, bucket_start) DO UPDATE SET
            sample_count = EXCLUDED.sample_count,
            avg_temperature = EXCLUDED.avg_temperature,
            min_temperature = EXCLUDED.min_temperature,
            max_temperature = EXCLUDED.max_temperature,
            avg_humidity = EXCLUDED.avg_humidity,
            min_humidity = EXCLUDED.min_humidity,
            max_humidity = EXCLUDED.max_humidity,
            avg_battery_level = EXCLUDED.avg_battery_level,
            min_battery_level = EXCLUDED.min_battery_level,
            max_battery_level = EXCLUDED.max_battery_level,
            updated_at = now()
        "#,
    )
    .bind::<Timestamptz, _>(since)
    .bind::<Timestamptz, _>(before)
    .execute(conn)
}

pub fn delete_older_than(
    conn: &mut PgConnection,
    cutoff: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(telemetry::table.filter(telemetry::received_at.lt(cutoff))).execute(conn)
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
