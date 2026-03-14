use std::sync::Arc;
use std::sync::atomic::Ordering;

use chrono::Utc;
use sysinfo::{Disks, Networks, System};
use tokio::time::{Duration, interval};
use tracing::{info, warn};

use crate::db::models::{NewAppMetric, NewServerMetric};
use crate::repositories::server_metrics_repo;
use crate::state::{AppState, DbPool};

/// Sample system-level metrics every 10 seconds and persist to the database.
pub async fn run_system_metrics_collector(db_pool: DbPool) {
    let mut sys = System::new_all();
    let mut networks = Networks::new_with_refreshed_list();
    let mut disks = Disks::new_with_refreshed_list();
    let mut prev_rx: u64 = networks.iter().map(|(_, n)| n.total_received()).sum();
    let mut prev_tx: u64 = networks.iter().map(|(_, n)| n.total_transmitted()).sum();
    let mut first_sample = true;

    let mut tick = interval(Duration::from_secs(10));
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
        let rx_delta = if first_sample { 0 } else { curr_rx.saturating_sub(prev_rx) };
        let tx_delta = if first_sample { 0 } else { curr_tx.saturating_sub(prev_tx) };
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
        let _ = tokio::task::spawn_blocking(move || {
            match pool.get() {
                Ok(mut conn) => {
                    if let Err(e) = server_metrics_repo::insert_server_metric(&mut conn, &record) {
                        warn!("Failed to insert server metric: {}", e);
                    }
                }
                Err(e) => warn!("Failed to get DB connection for server metrics: {}", e),
            }
        })
        .await;
    }
}

/// Flush accumulated HTTP and Zenoh metrics every 10 seconds.
pub async fn run_app_metrics_flusher(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(10));
    loop {
        tick.tick().await;

        let (req_count, err_count, latency_sum_micros, samples) =
            state.metrics_accumulator.drain();

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
        let _ = tokio::task::spawn_blocking(move || {
            match pool.get() {
                Ok(mut conn) => {
                    if let Err(e) = server_metrics_repo::insert_app_metric(&mut conn, &record) {
                        warn!("Failed to insert app metric: {}", e);
                    }
                }
                Err(e) => warn!("Failed to get DB connection for app metrics: {}", e),
            }
        })
        .await;
    }
}

/// Delete metrics older than 24 hours, running once per hour.
pub async fn run_metrics_retention(db_pool: DbPool) {
    let mut tick = interval(Duration::from_secs(3600));
    loop {
        tick.tick().await;

        let cutoff = Utc::now().naive_utc() - chrono::TimeDelta::hours(24);

        let pool = db_pool.clone();
        let _ = tokio::task::spawn_blocking(move || {
            match pool.get() {
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
            }
        })
        .await;
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
