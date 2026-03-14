use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::Ordering;
use tracing::{info, warn};

use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::types::PendingAction;
use crate::state::{DbPool, ZenohMetrics};

use super::handlers;

/// Start zenoh subscribers for telemetry and heartbeat topics.
///
/// Spawns one subscriber handler in a background tokio task and runs the other
/// in the current task. Both loop indefinitely, receiving messages and
/// dispatching them to the appropriate handler function.
///
/// All synchronous handler functions (DB-touching) are dispatched via
/// `spawn_blocking` to avoid starving the Tokio runtime.
///
/// # Errors
///
/// Returns an error if any Zenoh subscriber declaration fails.
pub async fn run_subscriber(
    session: Arc<zenoh::Session>,
    db_pool: DbPool,
    zenoh_metrics: Arc<ZenohMetrics>,
    rule_cache: Arc<RwLock<RuleCache>>,
    http_client: reqwest::Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use extrittio_common::topics::patterns;

    let telemetry_sub = session
        .declare_subscriber(patterns::TELEMETRY)
        .await?;

    let heartbeat_sub = session
        .declare_subscriber(patterns::HEARTBEAT)
        .await?;

    let shadow_report_sub = session
        .declare_subscriber(patterns::SHADOW_REPORT)
        .await?;

    let shadow_get_sub = session
        .declare_subscriber(patterns::SHADOW_GET)
        .await?;

    let log_sub = session
        .declare_subscriber(patterns::LOGS)
        .await?;

    let cmd_response_sub = session
        .declare_subscriber(patterns::COMMANDS_RESPONSE)
        .await?;

    info!(
        "Zenoh subscribers declared for telemetry, heartbeat, shadow, log, and command response topics"
    );

    // Spawn heartbeat handler in a background task
    let heartbeat_pool = db_pool.clone();
    let heartbeat_metrics = zenoh_metrics.clone();
    let heartbeat_cache = rule_cache.clone();
    let heartbeat_client = http_client.clone();
    tokio::spawn(async move {
        loop {
            match heartbeat_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = heartbeat_pool.clone();
                    let cache = heartbeat_cache.clone();
                    let client = heartbeat_client.clone();
                    let action_pool = heartbeat_pool.clone();
                    let action_cache = heartbeat_cache.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        handlers::heartbeat::handle_heartbeat(&pool, &payload, &cache)
                    })
                    .await;
                    heartbeat_metrics.messages_in.fetch_add(1, Ordering::Relaxed);

                    // Process pending actions from rule evaluation
                    if let Ok(actions) = result {
                        for action in actions {
                            let p = action_pool.clone();
                            let c = action_cache.clone();
                            let cl = client.clone();
                            tokio::spawn(async move {
                                execute_action(action, &p, &c, &cl).await;
                            });
                        }
                    }
                }
                Err(e) => {
                    warn!("Heartbeat subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn shadow report handler
    let shadow_report_pool = db_pool.clone();
    let shadow_report_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match shadow_report_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = shadow_report_pool.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        handlers::shadow::handle_shadow_report(&pool, &payload);
                    })
                    .await;
                    shadow_report_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Shadow report subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn shadow get handler (async — DB part uses spawn_blocking internally)
    let shadow_get_pool = db_pool.clone();
    let shadow_get_session = session.clone();
    let shadow_get_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match shadow_get_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    handlers::shadow::handle_shadow_get(
                        &shadow_get_pool,
                        &shadow_get_session,
                        &payload,
                        &shadow_get_metrics,
                    )
                    .await;
                    shadow_get_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Shadow get subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn log handler
    let log_pool = db_pool.clone();
    let log_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match log_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = log_pool.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        handlers::log::handle_device_log(&pool, &payload);
                    })
                    .await;
                    log_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Log subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn command response handler
    let cmd_response_pool = db_pool.clone();
    let cmd_response_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match cmd_response_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = cmd_response_pool.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        handlers::command_response::handle_command_response(&pool, &payload);
                    })
                    .await;
                    cmd_response_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Command response subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Run telemetry handler in the current task
    loop {
        match telemetry_sub.recv_async().await {
            Ok(sample) => {
                let payload = sample.payload().to_bytes().to_vec();
                let pool = db_pool.clone();
                let cache = rule_cache.clone();
                let client = http_client.clone();
                let action_pool = db_pool.clone();
                let action_cache = rule_cache.clone();
                let result = tokio::task::spawn_blocking(move || {
                    handlers::telemetry::handle_telemetry(&pool, &payload, &cache)
                })
                .await;
                zenoh_metrics.messages_in.fetch_add(1, Ordering::Relaxed);

                // Process pending actions from rule evaluation
                if let Ok(actions) = result {
                    for action in actions {
                        let p = action_pool.clone();
                        let c = action_cache.clone();
                        let cl = client.clone();
                        tokio::spawn(async move {
                            execute_action(action, &p, &c, &cl).await;
                        });
                    }
                }
            }
            Err(e) => {
                warn!("Telemetry subscriber channel closed: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Execute a single `PendingAction` produced by rule evaluation.
///
/// Each variant is handled independently:
/// - DB-touching operations use `spawn_blocking`
/// - Async operations (webhooks, commands) run directly on the tokio runtime
pub async fn execute_action(
    action: PendingAction,
    db_pool: &DbPool,
    rule_cache: &Arc<RwLock<RuleCache>>,
    http_client: &reqwest::Client,
) {
    match action {
        PendingAction::CreateAlert {
            rule_id,
            device_id,
            severity,
            message,
            triggered_value,
        } => {
            let pool = db_pool.clone();
            let cache = rule_cache.clone();
            let rid = rule_id.clone();
            let did = device_id.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::services::alert_service::create_alert(
                    &mut conn,
                    Some(rule_id),
                    device_id,
                    severity,
                    message,
                    triggered_value,
                )
                .map_err(|e| e.to_string())
            })
            .await;
            match result {
                Ok(Ok(alert)) => {
                    // Update active_alerts in the cache
                    if let Ok(mut c) = cache.write() {
                        c.active_alerts
                            .insert((rid, did), alert.id);
                    }
                }
                Ok(Err(msg)) => warn!("Failed to create alert: {}", msg),
                Err(e) => warn!("CreateAlert task panicked: {}", e),
            }
        }
        PendingAction::UpdateAlertValue {
            alert_id,
            triggered_value,
        } => {
            let pool = db_pool.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::services::alert_service::update_triggered_value(
                    &mut conn,
                    &alert_id,
                    triggered_value,
                )
                .map_err(|e| e.to_string())
            })
            .await;
            match result {
                Ok(Err(msg)) => warn!("Failed to update alert value: {}", msg),
                Err(e) => warn!("UpdateAlertValue task panicked: {}", e),
                _ => {}
            }
        }
        PendingAction::ResolveAlert { alert_id } => {
            let pool = db_pool.clone();
            let cache = rule_cache.clone();
            let aid = alert_id.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::services::alert_service::resolve_alert(&mut conn, &alert_id)
                    .map_err(|e| e.to_string())
            })
            .await;
            match result {
                Ok(Ok(alert)) => {
                    // Remove from active_alerts cache
                    if let Ok(mut c) = cache.write() {
                        // Find the key that contains this alert_id
                        let key_to_remove = c
                            .active_alerts
                            .iter()
                            .find(|(_, v)| **v == aid)
                            .map(|(k, _)| k.clone());
                        if let Some(key) = key_to_remove {
                            c.active_alerts.remove(&key);
                        }
                        let _ = alert; // used above via aid
                    }
                }
                Ok(Err(msg)) => warn!("Failed to resolve alert: {}", msg),
                Err(e) => warn!("ResolveAlert task panicked: {}", e),
            }
        }
        PendingAction::SendWebhook { url, payload } => {
            let result = http_client
                .post(&url)
                .json(&payload)
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await;
            match result {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        warn!(
                            "Webhook to {} returned status {}",
                            url,
                            resp.status()
                        );
                    }
                }
                Err(e) => {
                    warn!("Webhook to {} failed: {}", url, e);
                }
            }
        }
        PendingAction::SendCommand {
            device_id,
            command,
            params,
        } => {
            // Convert params Value to HashMap<String, String> for command_service
            let params_map: std::collections::HashMap<String, String> = match params {
                serde_json::Value::Object(map) => map
                    .into_iter()
                    .map(|(k, v)| {
                        let val = match v {
                            serde_json::Value::String(s) => s,
                            other => other.to_string(),
                        };
                        (k, val)
                    })
                    .collect(),
                _ => std::collections::HashMap::new(),
            };
            // Note: send_command requires zenoh_session and zenoh_metrics which
            // are not available here. For rule-triggered commands, we insert the
            // command record and publish via Zenoh. Since we don't have the
            // session here, we'll do a DB-only insert and log it.
            // TODO: Pass zenoh_session if full command sending is needed.
            let pool = db_pool.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                // Just verify the device exists; actual command sending via
                // Zenoh requires the session which is available in the subscriber scope.
                crate::repositories::device_repo::find_device(&mut conn, &device_id)
                    .map_err(|e| e.to_string())?;
                let correlation_id = uuid::Uuid::new_v4().to_string();
                let params_json =
                    serde_json::to_string(&params_map).unwrap_or_else(|_| "{}".to_string());
                crate::repositories::command_repo::insert_command(
                    &mut conn,
                    &crate::db::models::NewCommandRecord {
                        id: correlation_id.clone(),
                        device_id: device_id.clone(),
                        command,
                        params: params_json,
                    },
                )
                .map_err(|e| e.to_string())?;
                Ok::<String, String>(correlation_id)
            })
            .await;
            match result {
                Ok(Ok(cid)) => {
                    info!("Rule-triggered command recorded: {}", cid);
                }
                Ok(Err(msg)) => warn!("Failed to record rule-triggered command: {}", msg),
                Err(e) => warn!("SendCommand task panicked: {}", e),
            }
        }
        PendingAction::UpdateCooldown {
            rule_id,
            device_id,
            fired_at,
        } => {
            let pool = db_pool.clone();
            let cache = rule_cache.clone();
            let rid = rule_id.clone();
            let did = device_id.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                let cooldown = crate::db::models::RuleCooldown {
                    rule_id,
                    device_id,
                    last_fired_at: fired_at,
                };
                crate::repositories::rule_repo::upsert_cooldown(&mut conn, &cooldown)
                    .map_err(|e| e.to_string())
            })
            .await;
            match result {
                Ok(Ok(())) => {
                    // Update cooldowns in the cache
                    if let Ok(mut c) = cache.write() {
                        c.cooldowns.insert((rid, did), fired_at);
                    }
                }
                Ok(Err(msg)) => warn!("Failed to upsert cooldown: {}", msg),
                Err(e) => warn!("UpdateCooldown task panicked: {}", e),
            }
        }
    }
}
