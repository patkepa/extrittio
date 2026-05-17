use std::sync::Arc;
use std::sync::atomic::Ordering;

use chrono::Utc;
use sysinfo::{Disks, Networks, System};
use tokio::time::{Duration, interval};
use tracing::{info, warn};

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{AppMetric, ServerMetric};
use crate::db::models::{NewAppMetric, NewServerMetric};
use crate::error::AppError;
use crate::repositories::server_metrics_repo;
use crate::repositories::server_metrics_repo::{DownsampledAppMetric, DownsampledServerMetric};
use crate::state::{AppState, DbPool};

/// Data returned by `get_metrics_history`. Either raw or downsampled fields
/// are populated, depending on the requested resolution.
pub struct MetricsHistoryData {
    pub system_raw: Option<Vec<ServerMetric>>,
    pub app_raw: Option<Vec<AppMetric>>,
    pub system_downsampled: Option<Vec<DownsampledServerMetric>>,
    pub app_downsampled: Option<Vec<DownsampledAppMetric>>,
}

/// Sample system-level metrics and persist them to the database.
pub async fn run_system_metrics_collector(db_pool: DbPool, interval_secs: u64) {
    let interval_secs = interval_secs.max(1);
    info!(
        "System metrics collector started ({}s interval)",
        interval_secs
    );

    let mut sys = System::new_all();
    let mut networks = Networks::new_with_refreshed_list();
    let mut disks = Disks::new_with_refreshed_list();
    let mut prev_rx: u64 = networks.iter().map(|(_, n)| n.total_received()).sum();
    let mut prev_tx: u64 = networks.iter().map(|(_, n)| n.total_transmitted()).sum();
    let mut first_sample = true;

    let mut tick = interval(Duration::from_secs(interval_secs));
    loop {
        tick.tick().await;

        sys.refresh_cpu_usage();
        sys.refresh_memory();
        networks.refresh(false);

        let cpu = sys.global_cpu_usage();
        let mem_used = sys.used_memory() as i64;
        let mem_total = sys.total_memory() as i64;

        disks.refresh(false);
        let mut disk_used: i64 = 0;
        let mut disk_total: i64 = 0;
        for disk in disks.iter() {
            if disk.mount_point() == std::path::Path::new("/") {
                disk_total = disk.total_space() as i64;
                disk_used = (disk.total_space() - disk.available_space()) as i64;
                break;
            }
        }
        if disk_total == 0 {
            if let Some(disk) = disks.iter().next() {
                disk_total = disk.total_space() as i64;
                disk_used = (disk.total_space() - disk.available_space()) as i64;
            }
        }

        let curr_rx: u64 = networks.iter().map(|(_, n)| n.total_received()).sum();
        let curr_tx: u64 = networks.iter().map(|(_, n)| n.total_transmitted()).sum();
        let rx_delta = if first_sample {
            0
        } else {
            curr_rx.saturating_sub(prev_rx)
        };
        let tx_delta = if first_sample {
            0
        } else {
            curr_tx.saturating_sub(prev_tx)
        };
        prev_rx = curr_rx;
        prev_tx = curr_tx;
        first_sample = false;

        let load = System::load_average();

        let record = NewServerMetric {
            cpu_usage_percent: cpu,
            memory_used_bytes: mem_used,
            memory_total_bytes: mem_total,
            disk_used_bytes: disk_used,
            disk_total_bytes: disk_total,
            network_rx_bytes_delta: rx_delta as i64,
            network_tx_bytes_delta: tx_delta as i64,
            load_avg_1m: load.one as f32,
            load_avg_5m: load.five as f32,
            load_avg_15m: load.fifteen as f32,
        };

        let pool = db_pool.clone();
        let _ = tokio::task::spawn_blocking(move || match pool.get() {
            Ok(mut conn) => {
                if let Err(e) = server_metrics_repo::insert_server_metric(&mut conn, &record) {
                    warn!("Failed to insert server metric: {}", e);
                }
            }
            Err(e) => warn!("Failed to get DB connection for server metrics: {}", e),
        })
        .await;
    }
}

/// Flush accumulated HTTP and Zenoh metrics.
pub async fn run_app_metrics_flusher(state: Arc<AppState>, interval_secs: u64) {
    let interval_secs = interval_secs.max(1);
    info!("App metrics flusher started ({}s interval)", interval_secs);

    let mut tick = interval(Duration::from_secs(interval_secs));
    loop {
        tick.tick().await;

        let (req_count, err_count, latency_sum_micros, samples) = state.metrics_accumulator.drain();

        let zenoh_in = state.zenoh_metrics.messages_in.swap(0, Ordering::Relaxed);
        let zenoh_out = state.zenoh_metrics.messages_out.swap(0, Ordering::Relaxed);

        let pool_state = state.db_pool.state();
        let db_pool_active = pool_state.connections as i32 - pool_state.idle_connections as i32;
        let db_pool_idle = pool_state.idle_connections as i32;

        let avg_latency_ms = if req_count > 0 {
            (latency_sum_micros as f64 / req_count as f64 / 1000.0) as f32
        } else {
            0.0
        };

        let p95_latency_ms = compute_p95(&samples);

        let record = NewAppMetric {
            request_count: req_count as i32,
            error_count: err_count as i32,
            avg_latency_ms,
            p95_latency_ms,
            db_pool_active,
            db_pool_idle,
            zenoh_messages_in: zenoh_in as i32,
            zenoh_messages_out: zenoh_out as i32,
        };

        let pool = state.db_pool.clone();
        let _ = tokio::task::spawn_blocking(move || match pool.get() {
            Ok(mut conn) => {
                if let Err(e) = server_metrics_repo::insert_app_metric(&mut conn, &record) {
                    warn!("Failed to insert app metric: {}", e);
                }
            }
            Err(e) => warn!("Failed to get DB connection for app metrics: {}", e),
        })
        .await;
    }
}

/// Delete old metrics rows, running once per hour.
pub async fn run_metrics_retention(db_pool: DbPool, retention_hours: u64) {
    let retention_hours = retention_hours.max(1);
    info!("Metrics retention started ({}h retention)", retention_hours);

    let mut tick = interval(Duration::from_secs(3600));
    loop {
        tick.tick().await;

        #[allow(clippy::cast_possible_wrap)]
        let cutoff = Utc::now().naive_utc() - chrono::TimeDelta::hours(retention_hours as i64);

        let pool = db_pool.clone();
        let _ = tokio::task::spawn_blocking(move || match pool.get() {
            Ok(mut conn) => {
                match server_metrics_repo::delete_old_server_metrics(&mut conn, cutoff) {
                    Ok(n) if n > 0 => info!("Pruned {} old server_metrics rows", n),
                    Err(e) => warn!("Failed to prune server_metrics: {}", e),
                    _ => {}
                }
                match server_metrics_repo::delete_old_app_metrics(&mut conn, cutoff) {
                    Ok(n) if n > 0 => info!("Pruned {} old app_metrics rows", n),
                    Err(e) => warn!("Failed to prune app_metrics: {}", e),
                    _ => {}
                }
            }
            Err(e) => warn!("Failed to get DB connection for metrics retention: {}", e),
        })
        .await;
    }
}

/// Get the latest system and application metrics snapshot.
pub fn get_current_metrics(
    ctx: &RequestContext,
    conn: &mut diesel::PgConnection,
) -> Result<(Option<ServerMetric>, Option<AppMetric>), AppError> {
    policy::require(ctx, Permission::ReadServerMetrics)?;

    let system = server_metrics_repo::get_latest_server_metric(conn)?;
    let app = server_metrics_repo::get_latest_app_metric(conn)?;
    Ok((system, app))
}

/// Get metrics history for the given time range.
/// When `resolution_secs > 10`, returns downsampled data.
/// Otherwise returns raw data (limited to 10,000 rows per table).
pub fn get_metrics_history(
    ctx: &RequestContext,
    conn: &mut diesel::PgConnection,
    since: chrono::NaiveDateTime,
    resolution_secs: i64,
) -> Result<MetricsHistoryData, AppError> {
    policy::require(ctx, Permission::ReadServerMetrics)?;

    if resolution_secs > 10 {
        let system_downsampled =
            server_metrics_repo::list_server_metrics_downsampled(conn, since, resolution_secs)?;
        let app_downsampled =
            server_metrics_repo::list_app_metrics_downsampled(conn, since, resolution_secs)?;
        Ok(MetricsHistoryData {
            system_raw: None,
            app_raw: None,
            system_downsampled: Some(system_downsampled),
            app_downsampled: Some(app_downsampled),
        })
    } else {
        let limit = 10_000;
        let system_raw = server_metrics_repo::list_server_metrics(conn, since, limit)?;
        let app_raw = server_metrics_repo::list_app_metrics(conn, since, limit)?;
        Ok(MetricsHistoryData {
            system_raw: Some(system_raw),
            app_raw: Some(app_raw),
            system_downsampled: None,
            app_downsampled: None,
        })
    }
}

/// Compute the 95th-percentile latency from micro-second samples, returning milliseconds.
fn compute_p95(samples: &[u64]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() as f64) * 0.95).ceil() as usize;
    let idx = idx.saturating_sub(1).min(sorted.len() - 1);
    sorted[idx] as f32 / 1000.0
}
