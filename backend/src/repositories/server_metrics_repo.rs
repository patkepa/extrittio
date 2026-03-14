// Repository functions for server diagnostics metrics

use chrono::NaiveDateTime;
use diesel::sql_types::{BigInt, Float, Integer, Timestamp};
use diesel::SqliteConnection;
use diesel::prelude::*;
use serde::Serialize;

use crate::db::models::{AppMetric, NewAppMetric, NewServerMetric, ServerMetric};
use crate::db::schema::{app_metrics, server_metrics};

// ---------------------------------------------------------------------------
// Downsampled result types (used in API responses, hence Serialize)
// ---------------------------------------------------------------------------

#[derive(QueryableByName, Debug, Serialize)]
#[serde(crate = "serde")]
pub struct DownsampledServerMetric {
    #[diesel(sql_type = BigInt)]
    pub bucket: i64,
    #[diesel(sql_type = Float)]
    pub cpu_usage_percent: f32,
    #[diesel(sql_type = BigInt)]
    pub memory_used_bytes: i64,
    #[diesel(sql_type = BigInt)]
    pub memory_total_bytes: i64,
    #[diesel(sql_type = BigInt)]
    pub disk_used_bytes: i64,
    #[diesel(sql_type = BigInt)]
    pub disk_total_bytes: i64,
    #[diesel(sql_type = BigInt)]
    pub network_rx_bytes_delta: i64,
    #[diesel(sql_type = BigInt)]
    pub network_tx_bytes_delta: i64,
    #[diesel(sql_type = Float)]
    pub load_avg_1m: f32,
    #[diesel(sql_type = Float)]
    pub load_avg_5m: f32,
    #[diesel(sql_type = Float)]
    pub load_avg_15m: f32,
}

#[derive(QueryableByName, Debug, Serialize)]
#[serde(crate = "serde")]
pub struct DownsampledAppMetric {
    #[diesel(sql_type = BigInt)]
    pub bucket: i64,
    #[diesel(sql_type = Integer)]
    pub request_count: i32,
    #[diesel(sql_type = Integer)]
    pub error_count: i32,
    #[diesel(sql_type = Float)]
    pub avg_latency_ms: f32,
    #[diesel(sql_type = Float)]
    pub p95_latency_ms: f32,
    #[diesel(sql_type = Integer)]
    pub db_pool_active: i32,
    #[diesel(sql_type = Integer)]
    pub db_pool_idle: i32,
    #[diesel(sql_type = Integer)]
    pub zenoh_messages_in: i32,
    #[diesel(sql_type = Integer)]
    pub zenoh_messages_out: i32,
}

// ---------------------------------------------------------------------------
// Insert
// ---------------------------------------------------------------------------

pub fn insert_server_metric(
    conn: &mut SqliteConnection,
    record: &NewServerMetric,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(server_metrics::table)
        .values(record)
        .execute(conn)?;
    Ok(())
}

pub fn insert_app_metric(
    conn: &mut SqliteConnection,
    record: &NewAppMetric,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(app_metrics::table)
        .values(record)
        .execute(conn)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Get latest
// ---------------------------------------------------------------------------

pub fn get_latest_server_metric(
    conn: &mut SqliteConnection,
) -> Result<Option<ServerMetric>, diesel::result::Error> {
    server_metrics::table
        .order(server_metrics::recorded_at.desc())
        .select(ServerMetric::as_select())
        .first(conn)
        .optional()
}

pub fn get_latest_app_metric(
    conn: &mut SqliteConnection,
) -> Result<Option<AppMetric>, diesel::result::Error> {
    app_metrics::table
        .order(app_metrics::recorded_at.desc())
        .select(AppMetric::as_select())
        .first(conn)
        .optional()
}

// ---------------------------------------------------------------------------
// List (time-range)
// ---------------------------------------------------------------------------

pub fn list_server_metrics(
    conn: &mut SqliteConnection,
    since: NaiveDateTime,
    limit: i64,
) -> Result<Vec<ServerMetric>, diesel::result::Error> {
    server_metrics::table
        .filter(server_metrics::recorded_at.gt(since))
        .order(server_metrics::recorded_at.asc())
        .limit(limit)
        .select(ServerMetric::as_select())
        .load(conn)
}

pub fn list_app_metrics(
    conn: &mut SqliteConnection,
    since: NaiveDateTime,
    limit: i64,
) -> Result<Vec<AppMetric>, diesel::result::Error> {
    app_metrics::table
        .filter(app_metrics::recorded_at.gt(since))
        .order(app_metrics::recorded_at.asc())
        .limit(limit)
        .select(AppMetric::as_select())
        .load(conn)
}

// ---------------------------------------------------------------------------
// Downsampled (epoch-bucketed aggregation)
// ---------------------------------------------------------------------------

pub fn list_server_metrics_downsampled(
    conn: &mut SqliteConnection,
    since: NaiveDateTime,
    resolution_secs: i64,
) -> Result<Vec<DownsampledServerMetric>, diesel::result::Error> {
    let sql = "\
        SELECT \
            (CAST(strftime('%s', recorded_at) AS INTEGER) / ?) * ? AS bucket, \
            AVG(cpu_usage_percent) AS cpu_usage_percent, \
            AVG(memory_used_bytes) AS memory_used_bytes, \
            AVG(memory_total_bytes) AS memory_total_bytes, \
            AVG(disk_used_bytes) AS disk_used_bytes, \
            AVG(disk_total_bytes) AS disk_total_bytes, \
            SUM(network_rx_bytes_delta) AS network_rx_bytes_delta, \
            SUM(network_tx_bytes_delta) AS network_tx_bytes_delta, \
            AVG(load_avg_1m) AS load_avg_1m, \
            AVG(load_avg_5m) AS load_avg_5m, \
            AVG(load_avg_15m) AS load_avg_15m \
         FROM server_metrics \
         WHERE recorded_at > ? \
         GROUP BY bucket \
         ORDER BY bucket ASC";

    diesel::sql_query(sql)
        .bind::<BigInt, _>(resolution_secs)
        .bind::<BigInt, _>(resolution_secs)
        .bind::<Timestamp, _>(since)
        .load(conn)
}

pub fn list_app_metrics_downsampled(
    conn: &mut SqliteConnection,
    since: NaiveDateTime,
    resolution_secs: i64,
) -> Result<Vec<DownsampledAppMetric>, diesel::result::Error> {
    let sql = "\
        SELECT \
            (CAST(strftime('%s', recorded_at) AS INTEGER) / ?) * ? AS bucket, \
            SUM(request_count) AS request_count, \
            SUM(error_count) AS error_count, \
            AVG(avg_latency_ms) AS avg_latency_ms, \
            MAX(p95_latency_ms) AS p95_latency_ms, \
            AVG(db_pool_active) AS db_pool_active, \
            AVG(db_pool_idle) AS db_pool_idle, \
            SUM(zenoh_messages_in) AS zenoh_messages_in, \
            SUM(zenoh_messages_out) AS zenoh_messages_out \
         FROM app_metrics \
         WHERE recorded_at > ? \
         GROUP BY bucket \
         ORDER BY bucket ASC";

    diesel::sql_query(sql)
        .bind::<BigInt, _>(resolution_secs)
        .bind::<BigInt, _>(resolution_secs)
        .bind::<Timestamp, _>(since)
        .load(conn)
}

// ---------------------------------------------------------------------------
// Retention cleanup
// ---------------------------------------------------------------------------

pub fn delete_old_server_metrics(
    conn: &mut SqliteConnection,
    older_than: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(server_metrics::table.filter(server_metrics::recorded_at.lt(older_than)))
        .execute(conn)
}

pub fn delete_old_app_metrics(
    conn: &mut SqliteConnection,
    older_than: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(app_metrics::table.filter(app_metrics::recorded_at.lt(older_than)))
        .execute(conn)
}
