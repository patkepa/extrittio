use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::Ordering;
use tokio::sync::Semaphore;
use tracing::{info, warn};

use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::types::PendingAction;
use crate::state::{DbPool, ZenohMetrics};

use super::handlers;

/// Maximum number of concurrent action-execution tasks to prevent unbounded
/// resource consumption when telemetry arrives faster than actions complete.
static ACTION_SEMAPHORE: std::sync::LazyLock<Arc<Semaphore>> =
    std::sync::LazyLock::new(|| Arc::new(Semaphore::new(64)));

/// Start zenoh subscribers for telemetry, heartbeat, shadow, log, and command
/// response topics.
///
/// Spawns five subscriber handlers in background tokio tasks (heartbeat,
/// shadow_report, shadow_get, log, command_response) and runs the telemetry
/// handler in the current task. All loop indefinitely, receiving messages and
/// dispatching them to the appropriate handler function.
///
/// Synchronous handler functions (DB-touching) are dispatched via
/// `spawn_blocking` to avoid starving the Tokio runtime, except where the
/// handler manages `spawn_blocking` internally (e.g. `shadow_get`).
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
    let heartbeat_session = session.clone();
    let heartbeat_zenoh_metrics = zenoh_metrics.clone();
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
                            let s = heartbeat_session.clone();
                            let m = heartbeat_zenoh_metrics.clone();
                            let permit = ACTION_SEMAPHORE.clone();
                            tokio::spawn(async move {
                                let _permit = permit.acquire().await;
                                execute_action(action, &p, &c, &cl, &s, &m).await;
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
                        let s = session.clone();
                        let m = zenoh_metrics.clone();
                        let permit = ACTION_SEMAPHORE.clone();
                        tokio::spawn(async move {
                            let _permit = permit.acquire().await;
                            execute_action(action, &p, &c, &cl, &s, &m).await;
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
    zenoh_session: &Arc<zenoh::Session>,
    zenoh_metrics: &Arc<ZenohMetrics>,
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

            // Atomically check-and-reserve to prevent duplicate alerts from
            // concurrent telemetry messages triggering the same rule+device.
            if let Ok(mut guard) = cache.write() {
                let key = (rid.clone(), did.clone());
                if guard.active_alerts.contains_key(&key) {
                    return;
                }
                // Reserve the slot; replaced with the real alert ID below.
                guard.active_alerts.insert(key, String::new());
            } else {
                warn!("Rule cache lock poisoned; skipping CreateAlert");
                return;
            }

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
                    if let Ok(mut c) = cache.write() {
                        c.active_alerts.insert((rid, did), alert.id);
                    }
                }
                Ok(Err(msg)) => {
                    // Roll back the reservation
                    if let Ok(mut c) = cache.write() {
                        c.active_alerts.remove(&(rid, did));
                    }
                    warn!("Failed to create alert: {}", msg);
                }
                Err(e) => {
                    if let Ok(mut c) = cache.write() {
                        c.active_alerts.remove(&(rid, did));
                    }
                    warn!("CreateAlert task panicked: {}", e);
                }
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
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::services::alert_service::resolve_alert(&mut conn, &alert_id)
                    .map_err(|e| e.to_string())
            })
            .await;
            match result {
                Ok(Ok(alert)) => {
                    if let Ok(mut c) = cache.write() {
                        if let Some(rule_id) = &alert.rule_id {
                            c.active_alerts
                                .remove(&(rule_id.clone(), alert.device_id.clone()));
                        }
                    }
                }
                Ok(Err(msg)) => warn!("Failed to resolve alert: {}", msg),
                Err(e) => warn!("ResolveAlert task panicked: {}", e),
            }
        }
        PendingAction::SendWebhook { url, headers, payload } => {
            // .json() already sets Content-Type: application/json
            let mut req = http_client.post(&url).json(&payload);
            for (k, v) in &headers {
                req = req.header(k, v);
            }
            let result = req
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
            let session = zenoh_session.clone();
            let metrics = zenoh_metrics.clone();
            let pool = db_pool.clone();
            let correlation_id = uuid::Uuid::new_v4().to_string();
            let params_json =
                serde_json::to_string(&params_map).unwrap_or_else(|_| "{}".to_string());
            let cmd_clone = command.clone();
            let did_clone = device_id.clone();
            let cid_clone = correlation_id.clone();
            // Persist to DB
            let db_result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::repositories::device_repo::find_device(&mut conn, &did_clone)
                    .map_err(|e| e.to_string())?;
                crate::repositories::command_repo::insert_command(
                    &mut conn,
                    &crate::db::models::NewCommandRecord {
                        id: cid_clone,
                        device_id: did_clone,
                        command: cmd_clone,
                        params: params_json,
                    },
                )
                .map_err(|e| e.to_string())
            })
            .await;
            match db_result {
                Ok(Ok(())) => {
                    // Publish via Zenoh
                    let proto_command = extrittio_common::extrittio::DeviceCommand {
                        command,
                        params: params_map,
                        correlation_id: correlation_id.clone(),
                    };
                    let payload = prost::Message::encode_to_vec(&proto_command);
                    let topic = extrittio_common::topics::commands(&device_id);
                    match session.put(&topic, payload).await {
                        Ok(()) => {
                            metrics.messages_out.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            info!("Rule-triggered command sent: {}", correlation_id);
                        }
                        Err(e) => {
                            warn!("Failed to publish rule-triggered command via Zenoh: {}", e);
                        }
                    }
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
        PendingAction::UpdateZoneEntry {
            rule_id,
            device_id,
            entered_at,
        } => {
            let cache = rule_cache.clone();
            let key = (rule_id, device_id);
            if let Ok(mut c) = cache.write() {
                match entered_at {
                    Some(ts) => {
                        c.zone_entry_times.insert(key, ts);
                    }
                    None => {
                        c.zone_entry_times.remove(&key);
                    }
                }
            } else {
                warn!("Rule cache lock poisoned; skipping UpdateZoneEntry");
            }
        }
    }
}
