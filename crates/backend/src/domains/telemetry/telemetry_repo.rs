// Repository functions for telemetry

use chrono::NaiveDateTime;
use diesel::OptionalExtension;
use diesel::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::{
    Array, BigInt, Bytea, Float4, Float8, Integer, Jsonb, Nullable, Text, Timestamptz,
};
use serde_json::Value as JsonValue;

use crate::db::models::{NewTelemetryRecord, TelemetryRecord, TelemetryRollupHourly};
use crate::db::schema::{telemetry, telemetry_rollups_hourly};

#[derive(QueryableByName)]
pub struct PartitionMaintenanceResult {
    #[diesel(sql_type = Integer)]
    pub created_count: i32,
    #[diesel(sql_type = Integer)]
    pub dropped_count: i32,
}

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

pub fn latest_telemetry(
    conn: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
) -> Result<Option<TelemetryRecord>, diesel::result::Error> {
    telemetry::table
        .filter(telemetry::tenant_id.eq(tenant_id))
        .filter(telemetry::device_id.eq(device_id))
        .order((telemetry::received_at.desc(), telemetry::id.desc()))
        .select(TelemetryRecord::as_select())
        .first(conn)
        .optional()
}

pub fn list_hourly_rollups(
    conn: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
    since: Option<NaiveDateTime>,
    before: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRollupHourly>, diesel::result::Error> {
    let mut query = telemetry_rollups_hourly::table
        .filter(telemetry_rollups_hourly::tenant_id.eq(tenant_id))
        .filter(telemetry_rollups_hourly::device_id.eq(device_id))
        .into_boxed();

    if let Some(since_dt) = since {
        query = query.filter(telemetry_rollups_hourly::bucket_start.ge(since_dt));
    }
    if let Some(before_dt) = before {
        query = query.filter(telemetry_rollups_hourly::bucket_start.lt(before_dt));
    }

    query
        .order(telemetry_rollups_hourly::bucket_start.desc())
        .limit(limit)
        .select(TelemetryRollupHourly::as_select())
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
) -> Result<(i64, NaiveDateTime), diesel::result::Error> {
    diesel::insert_into(telemetry::table)
        .values(record)
        .returning((telemetry::id, telemetry::received_at))
        .get_result(conn)
}

pub fn upsert_latest_state(
    conn: &mut PgConnection,
    record: &NewTelemetryRecord,
    telemetry_id: i64,
    received_at: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::sql_query(
        r#"
        INSERT INTO device_latest_state (
            tenant_id, device_id, telemetry_id, payload, temperature, humidity,
            battery_level, custom_json, latitude, longitude, speed, altitude,
            heading, received_at, updated_at
        )
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, now())
        ON CONFLICT (tenant_id, device_id) DO UPDATE SET
            telemetry_id = EXCLUDED.telemetry_id,
            payload = EXCLUDED.payload,
            temperature = EXCLUDED.temperature,
            humidity = EXCLUDED.humidity,
            battery_level = EXCLUDED.battery_level,
            custom_json = EXCLUDED.custom_json,
            latitude = EXCLUDED.latitude,
            longitude = EXCLUDED.longitude,
            speed = EXCLUDED.speed,
            altitude = EXCLUDED.altitude,
            heading = EXCLUDED.heading,
            received_at = EXCLUDED.received_at,
            updated_at = now()
        WHERE device_latest_state.received_at <= EXCLUDED.received_at
        "#,
    )
    .bind::<Text, _>(&record.tenant_id)
    .bind::<Text, _>(&record.device_id)
    .bind::<BigInt, _>(telemetry_id)
    .bind::<Bytea, _>(&record.payload)
    .bind::<Nullable<Float4>, _>(record.temperature)
    .bind::<Nullable<Float4>, _>(record.humidity)
    .bind::<Nullable<Float4>, _>(record.battery_level)
    .bind::<Nullable<Jsonb>, _>(&record.custom_json)
    .bind::<Nullable<Float8>, _>(record.latitude)
    .bind::<Nullable<Float8>, _>(record.longitude)
    .bind::<Nullable<Float4>, _>(record.speed)
    .bind::<Nullable<Float4>, _>(record.altitude)
    .bind::<Nullable<Float4>, _>(record.heading)
    .bind::<Timestamptz, _>(received_at)
    .execute(conn)
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

pub fn maintain_partitions(
    conn: &mut PgConnection,
    months_ahead: i32,
    cutoff: NaiveDateTime,
) -> Result<PartitionMaintenanceResult, diesel::result::Error> {
    diesel::sql_query(
        r#"
        SELECT
            ensure_telemetry_partitions($1) AS created_count,
            drop_telemetry_partitions_older_than($2) AS dropped_count
        "#,
    )
    .bind::<Integer, _>(months_ahead)
    .bind::<Timestamptz, _>(cutoff)
    .get_result(conn)
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
