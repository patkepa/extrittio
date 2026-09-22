use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use extrittio_backend_core::{
    AlertMaintenanceApplication, CommandWorkerApplication, DeviceIngressApplication,
    LogIngressApplication,
};

pub async fn run_metric_retention(
    application: extrittio_backend_core::application::MetricMaintenanceApplication,
    raw_days: u64,
    rollup_days: u64,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(3_600));
    info!(raw_days, rollup_days, "Metric retention started");
    loop {
        interval.tick().await;
        match application
            .prune(raw_days, rollup_days, chrono::Utc::now())
            .await
        {
            Ok(outcome) => {
                if outcome.events_deleted > 0
                    || outcome.rollups_deleted > 0
                    || outcome.receipts_deleted > 0
                {
                    info!(
                        events_deleted = outcome.events_deleted,
                        rollups_deleted = outcome.rollups_deleted,
                        receipts_deleted = outcome.receipts_deleted,
                        "Metric retention pruned expired data"
                    );
                }
            }
            Err(error) => warn!("Metric retention error: {error}"),
        }
    }
}

/// Compute a backoff sleep duration based on consecutive failures.
/// Doubles each failure from `base` up to `max`.
fn backoff_duration(base: Duration, consecutive_failures: u32, max: Duration) -> Duration {
    let multiplier = 2u64.saturating_pow(consecutive_failures.min(10));
    let backoff = Duration::from_secs(base.as_secs().saturating_mul(multiplier));
    backoff.min(max)
}

pub async fn run_offline_checker(
    application: DeviceIngressApplication,
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

        let result = application
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

pub async fn run_command_timeout_checker(application: CommandWorkerApplication, timeout_secs: u64) {
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

        let result = application.timeout_stale(timeout_secs).await;

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

pub async fn run_alert_retention(application: AlertMaintenanceApplication, retention_days: u64) {
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

        let result = application
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

pub async fn run_log_retention(application: LogIngressApplication, retention_days: u64) {
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

        let result = application.prune(retention_days).await;

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
