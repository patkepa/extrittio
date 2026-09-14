use std::sync::Arc;
use std::sync::atomic::Ordering;

use sysinfo::{Disks, Networks, System};
use tokio::time::{Duration, interval};
use tracing::{info, warn};

use crate::domains::operations::metrics_types::{NewAppMetricRecord, NewSystemMetricRecord};
use crate::state::AppState;
use extrittio_backend_core::application::MetricsWorkerApplication;

pub async fn run_system_metrics_collector(
    application: MetricsWorkerApplication,
    interval_secs: u64,
) {
    let interval_secs = interval_secs.max(1);
    info!(
        "System metrics collector started ({}s interval)",
        interval_secs
    );

    let mut system = System::new_all();
    let mut networks = Networks::new_with_refreshed_list();
    let mut disks = Disks::new_with_refreshed_list();
    let mut previous_rx: u64 = networks
        .values()
        .map(|network| network.total_received())
        .sum();
    let mut previous_tx: u64 = networks
        .values()
        .map(|network| network.total_transmitted())
        .sum();
    let mut first_sample = true;
    let mut tick = interval(Duration::from_secs(interval_secs));

    loop {
        tick.tick().await;
        system.refresh_cpu_usage();
        system.refresh_memory();
        networks.refresh(false);
        disks.refresh(false);

        let mut disk_used = 0;
        let mut disk_total = 0;
        for disk in disks.iter() {
            if disk.mount_point() == std::path::Path::new("/") {
                disk_total = disk.total_space() as i64;
                disk_used = (disk.total_space() - disk.available_space()) as i64;
                break;
            }
        }
        if disk_total == 0
            && let Some(disk) = disks.iter().next()
        {
            disk_total = disk.total_space() as i64;
            disk_used = (disk.total_space() - disk.available_space()) as i64;
        }

        let current_rx: u64 = networks
            .values()
            .map(|network| network.total_received())
            .sum();
        let current_tx: u64 = networks
            .values()
            .map(|network| network.total_transmitted())
            .sum();
        let rx_delta = if first_sample {
            0
        } else {
            current_rx.saturating_sub(previous_rx)
        };
        let tx_delta = if first_sample {
            0
        } else {
            current_tx.saturating_sub(previous_tx)
        };
        previous_rx = current_rx;
        previous_tx = current_tx;
        first_sample = false;
        let load = System::load_average();

        if let Err(error) = application
            .record_system(NewSystemMetricRecord {
                cpu_usage_percent: system.global_cpu_usage(),
                memory_used_bytes: system.used_memory() as i64,
                memory_total_bytes: system.total_memory() as i64,
                disk_used_bytes: disk_used,
                disk_total_bytes: disk_total,
                network_rx_bytes_delta: rx_delta as i64,
                network_tx_bytes_delta: tx_delta as i64,
                load_avg_1m: load.one as f32,
                load_avg_5m: load.five as f32,
                load_avg_15m: load.fifteen as f32,
            })
            .await
        {
            warn!(%error, "Failed to insert server metric");
        }
    }
}

pub async fn run_app_metrics_flusher(
    state: Arc<AppState>,
    application: MetricsWorkerApplication,
    interval_secs: u64,
) {
    let interval_secs = interval_secs.max(1);
    info!("App metrics flusher started ({}s interval)", interval_secs);
    let mut tick = interval(Duration::from_secs(interval_secs));

    loop {
        tick.tick().await;
        let (request_count, error_count, latency_sum_micros, samples) =
            state.metrics_accumulator.drain();
        let zenoh_in = state.zenoh_metrics.messages_in.swap(0, Ordering::Relaxed);
        let zenoh_out = state.zenoh_metrics.messages_out.swap(0, Ordering::Relaxed);
        let (db_pool_active, db_pool_idle) = state.database.connection_counts();
        let avg_latency_ms = if request_count > 0 {
            (latency_sum_micros as f64 / request_count as f64 / 1000.0) as f32
        } else {
            0.0
        };

        if let Err(error) = application
            .record_app(NewAppMetricRecord {
                request_count: request_count as i32,
                error_count: error_count as i32,
                avg_latency_ms,
                p95_latency_ms: compute_p95(&samples),
                db_pool_active,
                db_pool_idle,
                zenoh_messages_in: zenoh_in as i32,
                zenoh_messages_out: zenoh_out as i32,
            })
            .await
        {
            warn!(%error, "Failed to insert application metric");
        }
    }
}

pub async fn run_metrics_retention(application: MetricsWorkerApplication, retention_hours: u64) {
    let retention_hours = retention_hours.max(1);
    info!("Metrics retention started ({}h retention)", retention_hours);
    let mut tick = interval(Duration::from_secs(3600));

    loop {
        tick.tick().await;
        match application.retain(retention_hours).await {
            Ok((system, app)) => {
                if system > 0 {
                    info!("Pruned {} old server_metrics rows", system);
                }
                if app > 0 {
                    info!("Pruned {} old app_metrics rows", app);
                }
            }
            Err(error) => warn!(%error, "Failed to prune metrics"),
        }
    }
}

fn compute_p95(samples: &[u64]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() as f64) * 0.95).ceil() as usize;
    let index = index.saturating_sub(1).min(sorted.len() - 1);
    sorted[index] as f32 / 1000.0
}
