use std::sync::{Arc, RwLock};
use std::time::Duration;

use diesel::PgConnection;
use prost::Message;
use tokio::time::sleep;
use tracing::{info, warn};

use super::cache::RuleCache;
use super::types::PendingAction;
use crate::db::models::{NewRuleActionOutboxEvent, RuleActionOutboxEvent};
use crate::repositories::rule_action_outbox_repo;
use crate::state::{DbPool, ZenohMetrics};
use crate::tenancy::DEFAULT_TENANT_ID;

const OUTBOX_BATCH_SIZE: i64 = 32;

pub fn enqueue_pending_actions(
    conn: &mut PgConnection,
    actions: &[PendingAction],
) -> Result<usize, String> {
    let mut inserted = 0;

    for action in actions {
        let payload = serde_json::to_value(action)
            .map_err(|e| format!("failed to serialize pending action: {e}"))?;
        let event = NewRuleActionOutboxEvent {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id_for_action(action).to_string(),
            event_type: event_type_for_action(action).to_string(),
            aggregate_type: aggregate_type_for_action(action).to_string(),
            aggregate_id: aggregate_id_for_action(action),
            payload,
        };
        rule_action_outbox_repo::insert_event(conn, &event)
            .map_err(|e| format!("failed to insert rule action outbox event: {e}"))?;
        inserted += 1;
    }

    Ok(inserted)
}

pub async fn run_rule_action_outbox_worker(
    db_pool: DbPool,
    rule_cache: Arc<RwLock<RuleCache>>,
    http_client: reqwest::Client,
    zenoh_session: Arc<zenoh::Session>,
    zenoh_metrics: Arc<ZenohMetrics>,
) {
    let worker_id = format!("rule-action-worker-{}", uuid::Uuid::new_v4());
    info!("Rule action outbox worker started: {}", worker_id);

    loop {
        let pool = db_pool.clone();
        let worker_id_for_claim = worker_id.clone();
        let claim_result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            rule_action_outbox_repo::claim_batch(&mut conn, &worker_id_for_claim, OUTBOX_BATCH_SIZE)
                .map_err(|e| e.to_string())
        })
        .await;

        let events = match claim_result {
            Ok(Ok(events)) => events,
            Ok(Err(e)) => {
                warn!("Failed to claim rule action outbox batch: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
            Err(e) => {
                warn!("Rule action outbox claim task panicked: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if events.is_empty() {
            sleep(Duration::from_secs(1)).await;
            continue;
        }

        for event in events {
            process_outbox_event(
                event,
                &db_pool,
                &rule_cache,
                &http_client,
                &zenoh_session,
                &zenoh_metrics,
            )
            .await;
        }
    }
}

async fn process_outbox_event(
    event: RuleActionOutboxEvent,
    db_pool: &DbPool,
    rule_cache: &Arc<RwLock<RuleCache>>,
    http_client: &reqwest::Client,
    zenoh_session: &Arc<zenoh::Session>,
    zenoh_metrics: &Arc<ZenohMetrics>,
) {
    let result = match serde_json::from_value::<PendingAction>(event.payload.clone()) {
        Ok(action) => {
            execute_action(
                action,
                db_pool,
                rule_cache,
                http_client,
                zenoh_session,
                zenoh_metrics,
            )
            .await
        }
        Err(e) => Err(format!("failed to deserialize pending action: {e}")),
    };

    let event_id = event.id.clone();
    let attempts = event.attempts;
    let max_attempts = event.max_attempts;
    let pool = db_pool.clone();
    let mark_result = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        match result {
            Ok(()) => rule_action_outbox_repo::mark_succeeded(&mut conn, &event_id)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Err(e) => rule_action_outbox_repo::mark_failed(
                &mut conn,
                &event_id,
                attempts,
                max_attempts,
                &e,
            )
            .map(|_| ())
            .map_err(|db_err| db_err.to_string()),
        }
    })
    .await;

    if let Err(e) = mark_result {
        warn!("Rule action outbox mark task panicked: {}", e);
    } else if let Ok(Err(e)) = mark_result {
        warn!("Failed to mark rule action outbox event: {}", e);
    }
}

pub async fn execute_action(
    action: PendingAction,
    db_pool: &DbPool,
    rule_cache: &Arc<RwLock<RuleCache>>,
    http_client: &reqwest::Client,
    zenoh_session: &Arc<zenoh::Session>,
    zenoh_metrics: &Arc<ZenohMetrics>,
) -> Result<(), String> {
    match action {
        PendingAction::CreateAlert {
            tenant_id,
            rule_id,
            device_id,
            severity,
            message,
            triggered_value,
        } => {
            let tid = tenant_id.clone();
            let rid = rule_id.clone();
            let did = device_id.clone();

            if let Ok(mut guard) = rule_cache.write() {
                let key = (tid.clone(), rid.clone(), did.clone());
                if guard.active_alerts.contains_key(&key) {
                    return Ok(());
                }
                guard.active_alerts.insert(key, String::new());
            } else {
                return Err("rule cache lock poisoned while reserving alert".to_string());
            }

            let pool = db_pool.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::services::alert_service::create_alert_for_tenant(
                    &mut conn,
                    &tenant_id,
                    Some(rule_id),
                    device_id,
                    severity,
                    message,
                    triggered_value,
                )
                .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())?;

            match result {
                Ok(alert) => {
                    if let Ok(mut cache) = rule_cache.write() {
                        cache.active_alerts.insert((tid, rid, did), alert.id);
                    }
                    Ok(())
                }
                Err(e) => {
                    if let Ok(mut cache) = rule_cache.write() {
                        cache.active_alerts.remove(&(tid, rid, did));
                    }
                    Err(e)
                }
            }
        }
        PendingAction::UpdateAlertValue {
            alert_id,
            triggered_value,
        } => {
            let pool = db_pool.clone();
            tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::services::alert_service::update_triggered_value(
                    &mut conn,
                    &alert_id,
                    triggered_value,
                )
                .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())?
        }
        PendingAction::ResolveAlert { alert_id } => {
            let cache = rule_cache.clone();
            let pool = db_pool.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::services::alert_service::resolve_alert_for_tenant(
                    &mut conn,
                    DEFAULT_TENANT_ID,
                    &alert_id,
                )
                .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())?;

            match result {
                Ok(alert) => {
                    if let Ok(mut cache) = cache.write() {
                        if let Some(rule_id) = &alert.rule_id {
                            cache.active_alerts.remove(&(
                                alert.tenant_id.clone(),
                                rule_id.clone(),
                                alert.device_id.clone(),
                            ));
                        }
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        PendingAction::SendWebhook {
            url,
            headers,
            payload,
        } => {
            let mut req = http_client.post(&url).json(&payload);
            for (key, value) in &headers {
                req = req.header(key, value);
            }
            let resp = req
                .timeout(Duration::from_secs(10))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if resp.status().is_success() {
                Ok(())
            } else {
                Err(format!(
                    "webhook to {url} returned status {}",
                    resp.status()
                ))
            }
        }
        PendingAction::SendCommand {
            tenant_id,
            device_id,
            command,
            params,
        } => {
            let params_map: std::collections::HashMap<String, String> = match params {
                serde_json::Value::Object(map) => map
                    .into_iter()
                    .map(|(key, value)| {
                        let rendered = match value {
                            serde_json::Value::String(s) => s,
                            other => other.to_string(),
                        };
                        (key, rendered)
                    })
                    .collect(),
                _ => std::collections::HashMap::new(),
            };

            let correlation_id = uuid::Uuid::new_v4().to_string();
            let params_json =
                serde_json::to_string(&params_map).unwrap_or_else(|_| "{}".to_string());
            let pool = db_pool.clone();
            let db_tenant_id = tenant_id.clone();
            let db_device_id = device_id.clone();
            let db_command = command.clone();
            let db_correlation_id = correlation_id.clone();
            tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::repositories::device_repo::find_device_for_tenant(
                    &mut conn,
                    &db_tenant_id,
                    &db_device_id,
                )
                .map_err(|e| e.to_string())?;
                crate::repositories::command_repo::insert_command(
                    &mut conn,
                    &crate::db::models::NewCommandRecord {
                        id: db_correlation_id,
                        tenant_id: db_tenant_id,
                        device_id: db_device_id,
                        command: db_command,
                        params: params_json,
                    },
                )
                .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())??;

            let proto_command = extrittio_common::extrittio::DeviceCommand {
                command,
                params: params_map,
                correlation_id: correlation_id.clone(),
            };
            let payload = proto_command.encode_to_vec();
            let topic = extrittio_common::topics::commands(&device_id);
            zenoh_session
                .put(&topic, payload)
                .await
                .map_err(|e| e.to_string())?;
            zenoh_metrics
                .messages_out
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            info!("Rule-triggered command sent: {}", correlation_id);
            Ok(())
        }
        PendingAction::UpdateCooldown {
            tenant_id,
            rule_id,
            device_id,
            fired_at,
        } => {
            let pool = db_pool.clone();
            let cooldown = crate::db::models::RuleCooldown {
                tenant_id: tenant_id.clone(),
                rule_id: rule_id.clone(),
                device_id: device_id.clone(),
                last_fired_at: fired_at,
            };
            tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| e.to_string())?;
                crate::repositories::rule_repo::upsert_cooldown(&mut conn, &cooldown)
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())??;

            if let Ok(mut cache) = rule_cache.write() {
                cache
                    .cooldowns
                    .insert((tenant_id, rule_id, device_id), fired_at);
            }
            Ok(())
        }
        PendingAction::UpdateZoneEntry {
            tenant_id,
            rule_id,
            device_id,
            entered_at,
        } => {
            let key = (tenant_id, rule_id, device_id);
            if let Ok(mut cache) = rule_cache.write() {
                match entered_at {
                    Some(ts) => {
                        cache.zone_entry_times.insert(key, ts);
                    }
                    None => {
                        cache.zone_entry_times.remove(&key);
                    }
                }
                Ok(())
            } else {
                Err("rule cache lock poisoned while updating zone entry".to_string())
            }
        }
    }
}

fn tenant_id_for_action(action: &PendingAction) -> &str {
    match action {
        PendingAction::CreateAlert { tenant_id, .. }
        | PendingAction::SendCommand { tenant_id, .. }
        | PendingAction::UpdateCooldown { tenant_id, .. }
        | PendingAction::UpdateZoneEntry { tenant_id, .. } => tenant_id,
        PendingAction::UpdateAlertValue { .. }
        | PendingAction::ResolveAlert { .. }
        | PendingAction::SendWebhook { .. } => DEFAULT_TENANT_ID,
    }
}

fn event_type_for_action(action: &PendingAction) -> &'static str {
    match action {
        PendingAction::CreateAlert { .. } => "rule.create_alert",
        PendingAction::UpdateAlertValue { .. } => "rule.update_alert_value",
        PendingAction::ResolveAlert { .. } => "rule.resolve_alert",
        PendingAction::SendWebhook { .. } => "rule.send_webhook",
        PendingAction::SendCommand { .. } => "rule.send_command",
        PendingAction::UpdateCooldown { .. } => "rule.update_cooldown",
        PendingAction::UpdateZoneEntry { .. } => "rule.update_zone_entry",
    }
}

fn aggregate_type_for_action(action: &PendingAction) -> &'static str {
    match action {
        PendingAction::CreateAlert { .. }
        | PendingAction::UpdateAlertValue { .. }
        | PendingAction::ResolveAlert { .. } => "alert",
        PendingAction::SendWebhook { .. } => "webhook",
        PendingAction::SendCommand { .. } => "command",
        PendingAction::UpdateCooldown { .. } => "rule_cooldown",
        PendingAction::UpdateZoneEntry { .. } => "zone_entry",
    }
}

fn aggregate_id_for_action(action: &PendingAction) -> String {
    match action {
        PendingAction::CreateAlert {
            rule_id, device_id, ..
        }
        | PendingAction::UpdateCooldown {
            rule_id, device_id, ..
        }
        | PendingAction::UpdateZoneEntry {
            rule_id, device_id, ..
        } => format!("{rule_id}:{device_id}"),
        PendingAction::UpdateAlertValue { alert_id, .. }
        | PendingAction::ResolveAlert { alert_id } => alert_id.clone(),
        PendingAction::SendWebhook { url, .. } => url.clone(),
        PendingAction::SendCommand {
            device_id, command, ..
        } => format!("{device_id}:{command}"),
    }
}
