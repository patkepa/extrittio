// Repository functions for server diagnostics metrics

use chrono::NaiveDateTime;
use diesel::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Float, Integer, Timestamptz};
use serde::Serialize;

use crate::models::{AppMetric, NewAppMetric, NewServerMetric, ServerMetric};
use crate::schema::{app_metrics, server_metrics};

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
    conn: &mut PgConnection,
    record: &NewServerMetric,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(server_metrics::table)
        .values(record)
        .execute(conn)?;
    Ok(())
}

pub fn insert_app_metric(
    conn: &mut PgConnection,
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
    conn: &mut PgConnection,
) -> Result<Option<ServerMetric>, diesel::result::Error> {
    server_metrics::table
        .order((
            server_metrics::recorded_at.desc(),
            server_metrics::id.desc(),
        ))
        .select(ServerMetric::as_select())
        .first(conn)
        .optional()
}

pub fn get_latest_app_metric(
    conn: &mut PgConnection,
) -> Result<Option<AppMetric>, diesel::result::Error> {
    app_metrics::table
        .order((app_metrics::recorded_at.desc(), app_metrics::id.desc()))
        .select(AppMetric::as_select())
        .first(conn)
        .optional()
}

// ---------------------------------------------------------------------------
// List (time-range)
// ---------------------------------------------------------------------------

pub fn list_server_metrics(
    conn: &mut PgConnection,
    since: NaiveDateTime,
    limit: i64,
) -> Result<Vec<ServerMetric>, diesel::result::Error> {
    server_metrics::table
        .filter(server_metrics::recorded_at.ge(since))
        .order((server_metrics::recorded_at.asc(), server_metrics::id.asc()))
        .limit(limit)
        .select(ServerMetric::as_select())
        .load(conn)
}

pub fn list_app_metrics(
    conn: &mut PgConnection,
    since: NaiveDateTime,
    limit: i64,
) -> Result<Vec<AppMetric>, diesel::result::Error> {
    app_metrics::table
        .filter(app_metrics::recorded_at.ge(since))
        .order((app_metrics::recorded_at.asc(), app_metrics::id.asc()))
        .limit(limit)
        .select(AppMetric::as_select())
        .load(conn)
}

// ---------------------------------------------------------------------------
// Downsampled (epoch-bucketed aggregation)
// ---------------------------------------------------------------------------

pub fn list_server_metrics_downsampled(
    conn: &mut PgConnection,
    since: NaiveDateTime,
    resolution_secs: i64,
) -> Result<Vec<DownsampledServerMetric>, diesel::result::Error> {
    let sql = "\
        SELECT \
            (FLOOR(EXTRACT(EPOCH FROM recorded_at) / $1::NUMERIC) * $1)::BIGINT AS bucket, \
            AVG(cpu_usage_percent)::REAL AS cpu_usage_percent, \
            (SUM(memory_used_bytes)::BIGINT / COUNT(*)) AS memory_used_bytes, \
            (SUM(memory_total_bytes)::BIGINT / COUNT(*)) AS memory_total_bytes, \
            (SUM(disk_used_bytes)::BIGINT / COUNT(*)) AS disk_used_bytes, \
            (SUM(disk_total_bytes)::BIGINT / COUNT(*)) AS disk_total_bytes, \
            SUM(network_rx_bytes_delta)::BIGINT AS network_rx_bytes_delta, \
            SUM(network_tx_bytes_delta)::BIGINT AS network_tx_bytes_delta, \
            AVG(load_avg_1m)::REAL AS load_avg_1m, \
            AVG(load_avg_5m)::REAL AS load_avg_5m, \
            AVG(load_avg_15m)::REAL AS load_avg_15m \
         FROM server_metrics \
         WHERE recorded_at >= $2 \
         GROUP BY bucket \
         ORDER BY bucket ASC";

    diesel::sql_query(sql)
        .bind::<BigInt, _>(resolution_secs)
        .bind::<Timestamptz, _>(since)
        .load(conn)
}

pub fn list_app_metrics_downsampled(
    conn: &mut PgConnection,
    since: NaiveDateTime,
    resolution_secs: i64,
) -> Result<Vec<DownsampledAppMetric>, diesel::result::Error> {
    let sql = "\
        SELECT \
            (FLOOR(EXTRACT(EPOCH FROM recorded_at) / $1::NUMERIC) * $1)::BIGINT AS bucket, \
            SUM(request_count)::INTEGER AS request_count, \
            SUM(error_count)::INTEGER AS error_count, \
            AVG(avg_latency_ms)::REAL AS avg_latency_ms, \
            MAX(p95_latency_ms) AS p95_latency_ms, \
            (SUM(db_pool_active) / COUNT(*))::INTEGER AS db_pool_active, \
            (SUM(db_pool_idle) / COUNT(*))::INTEGER AS db_pool_idle, \
            SUM(zenoh_messages_in)::INTEGER AS zenoh_messages_in, \
            SUM(zenoh_messages_out)::INTEGER AS zenoh_messages_out \
         FROM app_metrics \
         WHERE recorded_at >= $2 \
         GROUP BY bucket \
         ORDER BY bucket ASC";

    diesel::sql_query(sql)
        .bind::<BigInt, _>(resolution_secs)
        .bind::<Timestamptz, _>(since)
        .load(conn)
}

// ---------------------------------------------------------------------------
// Retention cleanup
// ---------------------------------------------------------------------------

pub fn delete_old_server_metrics(
    conn: &mut PgConnection,
    older_than: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(server_metrics::table.filter(server_metrics::recorded_at.lt(older_than)))
        .execute(conn)
}

pub fn delete_old_app_metrics(
    conn: &mut PgConnection,
    older_than: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(app_metrics::table.filter(app_metrics::recorded_at.lt(older_than))).execute(conn)
}
