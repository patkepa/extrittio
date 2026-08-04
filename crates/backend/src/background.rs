use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use chrono::Timelike;
use diesel::Connection;
use tracing::{info, warn};

use crate::error::AppError;
use crate::repositories::{device_repo, network_observed_host_repo};
use crate::rule_engine::actions::enqueue_pending_actions;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::evaluate::evaluate_status_change_for_tenant;
use crate::rule_engine::types::StatusChange;
use crate::services::{command_service, device_service, log_service, telemetry_service};
use crate::state::DbPool;

/// Compute a backoff sleep duration based on consecutive failures.
/// Doubles each failure from `base` up to `max`.
fn backoff_duration(base: Duration, consecutive_failures: u32, max: Duration) -> Duration {
    let multiplier = 2u64.saturating_pow(consecutive_failures.min(10));
    let backoff = Duration::from_secs(base.as_secs().saturating_mul(multiplier));
    backoff.min(max)
}

pub async fn run_offline_checker(
    db_pool: DbPool,
    timeout_secs: u64,
    rule_cache: Arc<RwLock<RuleCache>>,
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

        let pool = db_pool.clone();
        let cache = rule_cache.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            conn.transaction::<(usize, usize), AppError, _>(|conn| {
                // Device transitions, transition logs, and rule outbox events
                // share one commit boundary.
                #[allow(clippy::cast_possible_wrap)]
                let cutoff = chrono::Utc::now().naive_utc()
                    - chrono::TimeDelta::seconds(timeout_secs as i64);
                let going_offline = device_repo::find_devices_going_offline(conn, cutoff)?;
                let count = device_service::mark_devices_offline(conn, &going_offline)?;

                let mut device_records = Vec::new();
                for identity in &going_offline {
                    device_records.push(device_repo::find_device_for_tenant(
                        conn,
                        identity.tenant_id_str(),
                        identity.device_id(),
                    )?);
                }

                let cache_guard = cache.read().map_err(|error| {
                    AppError::Internal(format!("failed to read-lock rule cache: {error}"))
                })?;
                let mut all_actions = Vec::new();
                for device in &device_records {
                    let change = StatusChange {
                        old_status: "online".to_string(),
                        new_status: "offline".to_string(),
                    };
                    let actions = evaluate_status_change_for_tenant(
                        &device.tenant_id,
                        &device.id,
                        device.device_type_id,
                        device.fleet_id,
                        &change,
                        &cache_guard,
                    );
                    all_actions.extend(actions);
                }

                let enqueued = enqueue_pending_actions(conn, &all_actions)?;
                Ok((count, enqueued))
            })
            .map_err(|error| error.to_string())
        })
        .await;

        match result {
            Ok(Ok((count, enqueued))) => {
                consecutive_failures = 0;
                if count > 0 {
                    info!(
                        "Marked {} devices as offline and enqueued {} rule action(s)",
                        count, enqueued
                    );
                }
            }
            Ok(Err(msg)) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Offline checker: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        msg,
                    );
                } else {
                    warn!("Offline checker error: {}", msg);
                }
            }
            Err(e) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Offline checker: {} consecutive failures (next retry in {}s): task panicked: {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        e,
                    );
                } else {
                    warn!("Offline checker task panicked: {}", e);
                }
            }
        }
    }
}

pub async fn run_command_timeout_checker(db_pool: DbPool, timeout_secs: u64) {
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

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            command_service::timeout_stale(&mut conn, timeout_secs).map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(count)) => {
                consecutive_failures = 0;
                if count > 0 {
                    info!("Marked {} commands as timed_out", count);
                }
            }
            Ok(Err(msg)) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Command timeout checker: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        msg,
                    );
                } else {
                    warn!("Command timeout checker error: {}", msg);
                }
            }
            Err(e) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Command timeout checker: {} consecutive failures (next retry in {}s): task panicked: {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        e,
                    );
                } else {
                    warn!("Command timeout checker task panicked: {}", e);
                }
            }
        }
    }
}

pub async fn run_alert_retention(db_pool: DbPool, retention_days: u64) {
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

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            #[allow(clippy::cast_possible_wrap)]
            let cutoff =
                chrono::Utc::now().naive_utc() - chrono::Duration::days(retention_days as i64);
            let alert_count =
                crate::services::alert_service::delete_resolved_older_than(&mut conn, cutoff)
                    .map_err(|e| e.to_string())?;

            // Prune stale cooldowns (older than max cooldown window of 24h)
            let cooldown_cutoff = chrono::Utc::now().naive_utc() - chrono::Duration::seconds(86400);
            let cooldown_count =
                crate::services::rule_service::delete_stale_cooldowns(&mut conn, cooldown_cutoff)
                    .map_err(|e| e.to_string())?;

            let observed_host_cutoff = chrono::Utc::now().naive_utc() - chrono::Duration::days(30);
            let observed_host_count =
                network_observed_host_repo::delete_older_than(&mut conn, observed_host_cutoff)
                    .map_err(|e| e.to_string())?;

            Ok::<(usize, usize, usize), String>((alert_count, cooldown_count, observed_host_count))
        })
        .await;

        match result {
            Ok(Ok((alert_count, cooldown_count, observed_host_count))) => {
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
                if observed_host_count > 0 {
                    info!(
                        "Network observed host retention: deleted {} stale hosts",
                        observed_host_count
                    );
                }
            }
            Ok(Err(msg)) => {
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
            Err(e) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Alert retention: {} consecutive failures (next retry in {}s): task panicked: {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        e,
                    );
                } else {
                    warn!("Alert retention task panicked: {}", e);
                }
            }
        }
    }
}

pub async fn run_log_retention(db_pool: DbPool, retention_days: u64) {
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

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            #[allow(clippy::cast_possible_wrap)]
            let cutoff =
                chrono::Utc::now().naive_utc() - chrono::Duration::days(retention_days as i64);
            log_service::delete_older_than(&mut conn, cutoff).map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(count)) => {
                consecutive_failures = 0;
                if count > 0 {
                    info!("Log retention: deleted {} device log rows", count);
                }
            }
            Ok(Err(msg)) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Log retention: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        msg,
                    );
                } else {
                    warn!("Log retention error: {}", msg);
                }
            }
            Err(e) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Log retention: {} consecutive failures (next retry in {}s): task panicked: {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        e,
                    );
                } else {
                    warn!("Log retention task panicked: {}", e);
                }
            }
        }
    }
}

pub async fn run_telemetry_rollup_and_retention(db_pool: DbPool, retention_days: u64) {
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

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            let now = chrono::Utc::now().naive_utc();
            let current_hour = now
                - chrono::Duration::minutes(i64::from(now.minute()))
                - chrono::Duration::seconds(i64::from(now.second()))
                - chrono::Duration::nanoseconds(i64::from(now.nanosecond()));
            let since = current_hour - chrono::Duration::hours(25);

            let rollup_count =
                telemetry_service::upsert_hourly_rollups(&mut conn, since, current_hour)
                    .map_err(|e| e.to_string())?;

            #[allow(clippy::cast_possible_wrap)]
            let cutoff = now - chrono::Duration::days(retention_days as i64);
            let partition_result = telemetry_service::maintain_partitions(&mut conn, 3, cutoff)
                .map_err(|e| e.to_string())?;
            let deleted_count = telemetry_service::delete_older_than(&mut conn, cutoff)
                .map_err(|e| e.to_string())?;

            Ok::<(usize, usize, i32, i32), String>((
                rollup_count,
                deleted_count,
                partition_result.created_count,
                partition_result.dropped_count,
            ))
        })
        .await;

        match result {
            Ok(Ok((rollup_count, deleted_count, created_partitions, dropped_partitions))) => {
                consecutive_failures = 0;
                if rollup_count > 0 {
                    info!("Telemetry rollup: upserted {} hourly buckets", rollup_count);
                }
                if created_partitions > 0 || dropped_partitions > 0 {
                    info!(
                        "Telemetry partitions: created {}, dropped {}",
                        created_partitions, dropped_partitions
                    );
                }
                if deleted_count > 0 {
                    info!("Telemetry retention: deleted {} raw rows", deleted_count);
                }
            }
            Ok(Err(msg)) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Telemetry rollup/retention: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        msg,
                    );
                } else {
                    warn!("Telemetry rollup/retention error: {}", msg);
                }
            }
            Err(e) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Telemetry rollup/retention: {} consecutive failures (next retry in {}s): task panicked: {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff)
                            .as_secs(),
                        e,
                    );
                } else {
                    warn!("Telemetry rollup/retention task panicked: {}", e);
                }
            }
        }
    }
}
