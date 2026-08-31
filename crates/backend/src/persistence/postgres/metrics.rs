use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime};

use crate::db::models::{AppMetric, NewAppMetric, NewServerMetric, ServerMetric};
use crate::domains::operations::metrics_repository::MetricsRepository;
use crate::domains::operations::metrics_types::{
    AppMetricRecord, MetricsHistory, MetricsSnapshot, NewAppMetricRecord, NewSystemMetricRecord,
    SystemMetricRecord,
};
use crate::persistence::PersistenceError;
use crate::repositories::server_metrics_repo;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

fn system_record(metric: ServerMetric) -> SystemMetricRecord {
    SystemMetricRecord {
        cpu_usage_percent: metric.cpu_usage_percent,
        memory_used_bytes: metric.memory_used_bytes,
        memory_total_bytes: metric.memory_total_bytes,
        disk_used_bytes: metric.disk_used_bytes,
        disk_total_bytes: metric.disk_total_bytes,
        network_rx_bytes_delta: metric.network_rx_bytes_delta,
        network_tx_bytes_delta: metric.network_tx_bytes_delta,
        load_avg_1m: metric.load_avg_1m,
        load_avg_5m: metric.load_avg_5m,
        load_avg_15m: metric.load_avg_15m,
        recorded_at: metric.recorded_at,
    }
}

fn app_record(metric: AppMetric) -> AppMetricRecord {
    AppMetricRecord {
        request_count: metric.request_count,
        error_count: metric.error_count,
        avg_latency_ms: metric.avg_latency_ms,
        p95_latency_ms: metric.p95_latency_ms,
        db_pool_active: metric.db_pool_active,
        db_pool_idle: metric.db_pool_idle,
        zenoh_messages_in: metric.zenoh_messages_in,
        zenoh_messages_out: metric.zenoh_messages_out,
        recorded_at: metric.recorded_at,
    }
}

fn bucket_time(bucket: i64) -> NaiveDateTime {
    DateTime::from_timestamp(bucket, 0)
        .unwrap_or_default()
        .naive_utc()
}

#[async_trait]
impl MetricsRepository for PostgresAdapter {
    async fn insert_system(&self, record: NewSystemMetricRecord) -> Result<(), PersistenceError> {
        self.executor
            .run(move |connection| {
                server_metrics_repo::insert_server_metric(
                    connection,
                    &NewServerMetric {
                        cpu_usage_percent: record.cpu_usage_percent,
                        memory_used_bytes: record.memory_used_bytes,
                        memory_total_bytes: record.memory_total_bytes,
                        disk_used_bytes: record.disk_used_bytes,
                        disk_total_bytes: record.disk_total_bytes,
                        network_rx_bytes_delta: record.network_rx_bytes_delta,
                        network_tx_bytes_delta: record.network_tx_bytes_delta,
                        load_avg_1m: record.load_avg_1m,
                        load_avg_5m: record.load_avg_5m,
                        load_avg_15m: record.load_avg_15m,
                    },
                )
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn insert_app(&self, record: NewAppMetricRecord) -> Result<(), PersistenceError> {
        self.executor
            .run(move |connection| {
                server_metrics_repo::insert_app_metric(
                    connection,
                    &NewAppMetric {
                        request_count: record.request_count,
                        error_count: record.error_count,
                        avg_latency_ms: record.avg_latency_ms,
                        p95_latency_ms: record.p95_latency_ms,
                        db_pool_active: record.db_pool_active,
                        db_pool_idle: record.db_pool_idle,
                        zenoh_messages_in: record.zenoh_messages_in,
                        zenoh_messages_out: record.zenoh_messages_out,
                    },
                )
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn current(&self) -> Result<MetricsSnapshot, PersistenceError> {
        self.executor
            .run(move |connection| {
                let system = server_metrics_repo::get_latest_server_metric(connection)
                    .map_err(map_diesel_error)?
                    .map(system_record);
                let app = server_metrics_repo::get_latest_app_metric(connection)
                    .map_err(map_diesel_error)?
                    .map(app_record);
                Ok(MetricsSnapshot { system, app })
            })
            .await
    }

    async fn history(
        &self,
        since: NaiveDateTime,
        resolution_secs: i64,
    ) -> Result<MetricsHistory, PersistenceError> {
        self.executor
            .run(move |connection| {
                if resolution_secs > 10 {
                    let system = server_metrics_repo::list_server_metrics_downsampled(
                        connection,
                        since,
                        resolution_secs,
                    )
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(|metric| SystemMetricRecord {
                        cpu_usage_percent: metric.cpu_usage_percent,
                        memory_used_bytes: metric.memory_used_bytes,
                        memory_total_bytes: metric.memory_total_bytes,
                        disk_used_bytes: metric.disk_used_bytes,
                        disk_total_bytes: metric.disk_total_bytes,
                        network_rx_bytes_delta: metric.network_rx_bytes_delta,
                        network_tx_bytes_delta: metric.network_tx_bytes_delta,
                        load_avg_1m: metric.load_avg_1m,
                        load_avg_5m: metric.load_avg_5m,
                        load_avg_15m: metric.load_avg_15m,
                        recorded_at: bucket_time(metric.bucket),
                    })
                    .collect();
                    let app = server_metrics_repo::list_app_metrics_downsampled(
                        connection,
                        since,
                        resolution_secs,
                    )
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(|metric| AppMetricRecord {
                        request_count: metric.request_count,
                        error_count: metric.error_count,
                        avg_latency_ms: metric.avg_latency_ms,
                        p95_latency_ms: metric.p95_latency_ms,
                        db_pool_active: metric.db_pool_active,
                        db_pool_idle: metric.db_pool_idle,
                        zenoh_messages_in: metric.zenoh_messages_in,
                        zenoh_messages_out: metric.zenoh_messages_out,
                        recorded_at: bucket_time(metric.bucket),
                    })
                    .collect();
                    Ok(MetricsHistory { system, app })
                } else {
                    let system =
                        server_metrics_repo::list_server_metrics(connection, since, 10_000)
                            .map_err(map_diesel_error)?
                            .into_iter()
                            .map(system_record)
                            .collect();
                    let app = server_metrics_repo::list_app_metrics(connection, since, 10_000)
                        .map_err(map_diesel_error)?
                        .into_iter()
                        .map(app_record)
                        .collect();
                    Ok(MetricsHistory { system, app })
                }
            })
            .await
    }

    async fn delete_before(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<(usize, usize), PersistenceError> {
        self.executor
            .run(move |connection| {
                let system = server_metrics_repo::delete_old_server_metrics(connection, cutoff)
                    .map_err(map_diesel_error)?;
                let app = server_metrics_repo::delete_old_app_metrics(connection, cutoff)
                    .map_err(map_diesel_error)?;
                Ok((system, app))
            })
            .await
    }
}
