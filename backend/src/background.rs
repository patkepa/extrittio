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

pub async fn run_offline_checker(
    db_pool: DbPool,
    timeout_secs: u64,
    rule_cache: Arc<RwLock<RuleCache>>,
    http_client: reqwest::Client,
    zenoh_session: Arc<zenoh::Session>,
    zenoh_metrics: Arc<crate::state::ZenohMetrics>,
) {
    let interval = Duration::from_secs(60); // check every minute
    info!("Offline checker started (timeout: {}s)", timeout_secs);

    loop {
        tokio::time::sleep(interval).await;

        let pool = db_pool.clone();
        let cache = rule_cache.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;

            // First, find which devices are going offline (before marking them)
            #[allow(clippy::cast_possible_wrap)]
            let cutoff =
                chrono::Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);
            let going_offline_ids =
                device_repo::find_devices_going_offline(&mut conn, cutoff)
                    .map_err(|e| e.to_string())?;

            // Now do the full check_offline_devices which marks them and logs
            let count = device_service::check_offline_devices(&mut conn, timeout_secs)
                .map_err(|e| e.to_string())?;

            // For each device going offline, load its record and evaluate rules
            let mut all_actions: Vec<PendingAction> = Vec::new();
            let cache_guard = cache.read().map_err(|e| e.to_string())?;

            for device_id in &going_offline_ids {
                if let Ok(device) = device_repo::find_device(&mut conn, device_id) {
                    // The device was just marked offline by check_offline_devices,
                    // so device.status is now "offline". We approximate the
                    // previous status as "online" for rule evaluation.
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
            drop(cache_guard);

            Ok::<(usize, Vec<PendingAction>), String>((count, all_actions))
        })
        .await;

        match result {
            Ok(Ok((count, actions))) => {
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
                warn!("Offline checker error: {}", msg);
            }
            Err(e) => {
                warn!("Offline checker task panicked: {}", e);
            }
        }
    }
}

pub async fn run_command_timeout_checker(db_pool: DbPool, timeout_secs: u64) {
    let interval = Duration::from_secs(30);
    info!(
        "Command timeout checker started (timeout: {}s)",
        timeout_secs
    );

    loop {
        tokio::time::sleep(interval).await;

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            command_service::timeout_stale(&mut conn, timeout_secs)
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(count)) => {
                if count > 0 {
                    info!("Marked {} commands as timed_out", count);
                }
            }
            Ok(Err(msg)) => {
                warn!("Command timeout checker error: {}", msg);
            }
            Err(e) => {
                warn!("Command timeout checker task panicked: {}", e);
            }
        }
    }
}

pub async fn run_alert_retention(db_pool: DbPool, retention_days: u64) {
    let interval = Duration::from_secs(3600); // check every hour
    info!("Alert retention started ({}d retention)", retention_days);

    loop {
        tokio::time::sleep(interval).await;

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
                crate::repositories::rule_repo::delete_cooldowns_older_than(&mut conn, cooldown_cutoff)
                    .map_err(|e| e.to_string())?;

            Ok::<(usize, usize), String>((alert_count, cooldown_count))
        })
        .await;

        match result {
            Ok(Ok((alert_count, cooldown_count))) => {
                if alert_count > 0 {
                    info!("Alert retention: deleted {} resolved alerts", alert_count);
                }
                if cooldown_count > 0 {
                    info!("Cooldown pruning: deleted {} stale cooldowns", cooldown_count);
                }
            }
            Ok(Err(msg)) => warn!("Alert retention error: {}", msg),
            Err(e) => warn!("Alert retention task panicked: {}", e),
        }
    }
}
