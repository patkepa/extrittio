use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tracing::{info, warn};

use crate::repositories::device_repo;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::evaluate::evaluate_status_change;
use crate::rule_engine::types::{PendingAction, StatusChange};
use crate::services::{command_service, device_service};
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
    http_client: reqwest::Client,
    zenoh_session: Arc<zenoh::Session>,
    zenoh_metrics: Arc<crate::state::ZenohMetrics>,
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

            // Mark offline and collect the affected device IDs in one pass
            // using a shared cutoff to avoid TOCTOU races.
            #[allow(clippy::cast_possible_wrap)]
            let cutoff =
                chrono::Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);
            let going_offline_ids =
                device_repo::find_devices_going_offline(&mut conn, cutoff)
                    .map_err(|e| e.to_string())?;
            let count = device_service::mark_devices_offline(&mut conn, &going_offline_ids)
                .map_err(|e| e.to_string())?;

            // Load device records for rule evaluation (no lock held yet)
            let mut device_records = Vec::new();
            for device_id in &going_offline_ids {
                if let Ok(device) = device_repo::find_device(&mut conn, device_id) {
                    device_records.push((device_id.clone(), device));
                }
            }

            // Take the read lock only for rule evaluation, then drop it
            let mut all_actions: Vec<PendingAction> = Vec::new();
            if let Ok(cache_guard) = cache.read() {
                for (device_id, device) in &device_records {
                    let change = StatusChange {
                        old_status: "online".to_string(),
                        new_status: "offline".to_string(),
                    };
                    let actions = evaluate_status_change(
                        device_id,
                        device.device_type_id,
                        device.fleet_id,
                        &change,
                        &cache_guard,
                    );
                    all_actions.extend(actions);
                }
            }

            Ok::<(usize, Vec<PendingAction>), String>((count, all_actions))
        })
        .await;

        match result {
            Ok(Ok((count, actions))) => {
                consecutive_failures = 0;
                if count > 0 {
                    info!("Marked {} devices as offline", count);
                }
                // Execute pending actions from rule evaluation
                for action in actions {
                    let p = db_pool.clone();
                    let c = rule_cache.clone();
                    let cl = http_client.clone();
                    let s = zenoh_session.clone();
                    let m = zenoh_metrics.clone();
                    tokio::spawn(async move {
                        crate::zenoh_handler::subscriber::execute_action(
                            action, &p, &c, &cl, &s, &m,
                        )
                        .await;
                    });
                }
            }
            Ok(Err(msg)) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Offline checker: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff).as_secs(),
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
                        backoff_duration(base_interval, consecutive_failures, max_backoff).as_secs(),
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
            command_service::timeout_stale(&mut conn, timeout_secs)
                .map_err(|e| e.to_string())
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
                        backoff_duration(base_interval, consecutive_failures, max_backoff).as_secs(),
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
                        backoff_duration(base_interval, consecutive_failures, max_backoff).as_secs(),
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
            let cutoff = chrono::Utc::now().naive_utc()
                - chrono::Duration::days(retention_days as i64);
            let alert_count =
                crate::services::alert_service::delete_resolved_older_than(&mut conn, cutoff)
                    .map_err(|e| e.to_string())?;

            // Prune stale cooldowns (older than max cooldown window of 24h)
            let cooldown_cutoff =
                chrono::Utc::now().naive_utc() - chrono::Duration::seconds(86400);
            let cooldown_count =
                crate::services::rule_service::delete_stale_cooldowns(&mut conn, cooldown_cutoff)
                    .map_err(|e| e.to_string())?;

            Ok::<(usize, usize), String>((alert_count, cooldown_count))
        })
        .await;

        match result {
            Ok(Ok((alert_count, cooldown_count))) => {
                consecutive_failures = 0;
                if alert_count > 0 {
                    info!("Alert retention: deleted {} resolved alerts", alert_count);
                }
                if cooldown_count > 0 {
                    info!("Cooldown pruning: deleted {} stale cooldowns", cooldown_count);
                }
            }
            Ok(Err(msg)) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= 5 {
                    tracing::error!(
                        "Alert retention: {} consecutive failures (next retry in {}s): {}",
                        consecutive_failures,
                        backoff_duration(base_interval, consecutive_failures, max_backoff).as_secs(),
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
                        backoff_duration(base_interval, consecutive_failures, max_backoff).as_secs(),
                        e,
                    );
                } else {
                    warn!("Alert retention task panicked: {}", e);
                }
            }
        }
    }
}
