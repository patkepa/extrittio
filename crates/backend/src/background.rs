use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use crate::persistence::RepositorySet;

/// Compute a backoff sleep duration based on consecutive failures.
/// Doubles each failure from `base` up to `max`.
fn backoff_duration(base: Duration, consecutive_failures: u32, max: Duration) -> Duration {
    let multiplier = 2u64.saturating_pow(consecutive_failures.min(10));
    let backoff = Duration::from_secs(base.as_secs().saturating_mul(multiplier));
    backoff.min(max)
}

pub async fn run_offline_checker(
    persistence: RepositorySet,
    timeout_secs: u64,
    rule_cache: Arc<crate::rule_snapshots::RuleSnapshotStore>,
) {
    let base_interval = Duration::from_secs(60);
    let max_backoff = Duration::from_secs(600); // 10 minutes
    let mut consecutive_failures: u32 = 0;
    info!("Offline checker started (timeout: {}s)", timeout_secs);

    loop {
        let sleep_dur = if consecutive_failures == 0 {
            base_interval
        } else {
            backoff_duration(base_interval, consecutive_failures, max_backoff)
        };
        tokio::time::sleep(sleep_dur).await;

        let result = extrittio_backend_core::DeviceIngressApplication::new(
            persistence.device_ingress.clone(),
            std::sync::Arc::new(crate::auth::SystemClock),
        )
        .mark_offline_devices(timeout_secs, rule_cache.as_ref())
        .await;

        match result {
            Ok(outcome) => {
                consecutive_failures = 0;
                if outcome.devices_updated > 0 {
                    info!(
                        "Marked {} devices as offline and enqueued {} rule action(s)",
                        outcome.devices_updated, outcome.actions_enqueued
                    );
                }
            }
            Err(error) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Offline checker: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        error,
                    );
                } else {
                    warn!("Offline checker error: {}", error);
                }
            }
        }
    }
}

pub async fn run_command_timeout_checker(persistence: RepositorySet, timeout_secs: u64) {
    let base_interval = Duration::from_secs(30);
    let max_backoff = Duration::from_secs(300); // 5 minutes
    let mut consecutive_failures: u32 = 0;
    info!(
        "Command timeout checker started (timeout: {}s)",
        timeout_secs
    );

    loop {
        let sleep_dur = if consecutive_failures == 0 {
            base_interval
        } else {
            backoff_duration(base_interval, consecutive_failures, max_backoff)
        };
        tokio::time::sleep(sleep_dur).await;

        let result = extrittio_backend_core::CommandWorkerApplication::new(
            persistence.commands.clone(),
            std::sync::Arc::new(crate::auth::SystemClock),
        )
        .timeout_stale(timeout_secs)
        .await;

        match result {
            Ok(count) => {
                consecutive_failures = 0;
                if count > 0 {
                    info!("Marked {} commands as timed_out", count);
                }
            }
            Err(error) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Command timeout checker: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        error,
                    );
                } else {
                    warn!("Command timeout checker error: {}", error);
                }
            }
        }
    }
}

pub async fn run_alert_retention(persistence: RepositorySet, retention_days: u64) {
    let maintenance = extrittio_backend_core::AlertMaintenanceApplication::new(
        persistence.alerts.clone(),
        persistence.rules.clone(),
    );
    let base_interval = Duration::from_secs(3600);
    let max_backoff = Duration::from_secs(7200); // 2 hours
    let mut consecutive_failures: u32 = 0;
    info!("Alert retention started ({}d retention)", retention_days);

    loop {
        let sleep_dur = if consecutive_failures == 0 {
            base_interval
        } else {
            backoff_duration(base_interval, consecutive_failures, max_backoff)
        };
        tokio::time::sleep(sleep_dur).await;

        let result = maintenance
            .prune(retention_days, chrono::Utc::now().naive_utc())
            .await
            .map_err(|error| error.to_string());

        match result {
            Ok((alert_count, cooldown_count)) => {
                consecutive_failures = 0;
                if alert_count > 0 {
                    info!("Alert retention: deleted {} resolved alerts", alert_count);
                }
                if cooldown_count > 0 {
                    info!(
                        "Cooldown pruning: deleted {} stale cooldowns",
                        cooldown_count
                    );
                }
            }
            Err(msg) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Alert retention: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        msg,
                    );
                } else {
                    warn!("Alert retention error: {}", msg);
                }
            }
        }
    }
}

pub async fn run_log_retention(persistence: RepositorySet, retention_days: u64) {
    let base_interval = Duration::from_secs(3600);
    let max_backoff = Duration::from_secs(7200);
    let mut consecutive_failures: u32 = 0;
    info!("Log retention started ({}d retention)", retention_days);

    loop {
        let sleep_dur = if consecutive_failures == 0 {
            base_interval
        } else {
            backoff_duration(base_interval, consecutive_failures, max_backoff)
        };
        tokio::time::sleep(sleep_dur).await;

        let result = extrittio_backend_core::LogIngressApplication::new(
            persistence.logs.clone(),
            std::sync::Arc::new(crate::auth::SystemClock),
        )
        .prune(retention_days)
        .await;

        match result {
            Ok(count) => {
                consecutive_failures = 0;
                if count > 0 {
                    info!("Log retention: deleted {} device log rows", count);
                }
            }
            Err(error) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Log retention: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        error,
                    );
                } else {
                    warn!("Log retention error: {}", error);
                }
            }
        }
    }
}

pub async fn run_telemetry_rollup_and_retention(persistence: RepositorySet, retention_days: u64) {
    let base_interval = Duration::from_secs(3600);
    let max_backoff = Duration::from_secs(7200);
    let mut consecutive_failures: u32 = 0;
    info!(
        "Telemetry rollup and retention started ({}d retention)",
        retention_days
    );

    loop {
        let sleep_dur = if consecutive_failures == 0 {
            base_interval
        } else {
            backoff_duration(base_interval, consecutive_failures, max_backoff)
        };
        tokio::time::sleep(sleep_dur).await;

        let result = extrittio_backend_core::TelemetryMaintenanceApplication::new(
            persistence.telemetry.clone(),
            std::sync::Arc::new(crate::auth::SystemClock),
        )
        .maintain(retention_days)
        .await;

        match result {
            Ok(outcome) => {
                consecutive_failures = 0;
                if outcome.rollups_upserted > 0 {
                    info!(
                        "Telemetry rollup: upserted {} hourly buckets",
                        outcome.rollups_upserted
                    );
                }
                if outcome.partitions.created_count > 0 || outcome.partitions.dropped_count > 0 {
                    info!(
                        "Telemetry partitions: created {}, dropped {}",
                        outcome.partitions.created_count, outcome.partitions.dropped_count
                    );
                }
                if outcome.rows_deleted > 0 {
                    info!(
                        "Telemetry retention: deleted {} raw rows",
                        outcome.rows_deleted
                    );
                }
            }
            Err(error) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Telemetry rollup/retention: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        error,
                    );
                } else {
                    warn!("Telemetry rollup/retention error: {}", error);
                }
            }
        }
    }
}
