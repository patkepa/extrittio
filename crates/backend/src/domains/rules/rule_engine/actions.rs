use std::sync::{Arc, RwLock};
use std::time::Duration;

use opentelemetry::global;
use opentelemetry::propagation::Injector;
use prost::Message;
use sha2::{Digest, Sha256};
use tokio::time::sleep;
use tracing::{Instrument, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::cache::RuleCache;
use super::types::PendingAction;
use crate::domains::alerts::types::{
    AlertTransition, AlertTransitionOutcome, CooldownRecord, NewAlertRecord,
};
use crate::domains::commands::types::NewCommandRecord;
use crate::domains::operations::outbox_types::{NewOutboxEventRecord, OutboxEventRecord};
use crate::persistence::Persistence;
use crate::state::ZenohMetrics;
use crate::tenancy::TenantId;

#[derive(Debug, Clone, Copy)]
pub struct OutboxWorkerConfig {
    pub batch_size: i64,
    pub concurrency: usize,
    pub idle_interval: Duration,
    pub lease_timeout: Duration,
}

pub(crate) fn outbox_event_for_action(
    action: &PendingAction,
) -> Result<NewOutboxEventRecord, serde_json::Error> {
    Ok(NewOutboxEventRecord {
        id: uuid::Uuid::new_v4().to_string(),
        tenant_id: tenant_id_for_action(action).to_string(),
        event_type: event_type_for_action(action).to_string(),
        aggregate_type: aggregate_type_for_action(action).to_string(),
        aggregate_id: aggregate_id_for_action(action),
        idempotency_key: Some(idempotency_key_for_action(action)),
        payload: serde_json::to_value(action)?,
    })
}

pub async fn run_rule_action_outbox_worker(
    persistence: Persistence,
    rule_cache: Arc<RwLock<RuleCache>>,
    http_client: reqwest::Client,
    zenoh_session: Arc<zenoh::Session>,
    zenoh_metrics: Arc<ZenohMetrics>,
    config: OutboxWorkerConfig,
) {
    let worker_id = format!("rule-action-worker-{}", uuid::Uuid::new_v4());
    info!("Rule action outbox worker started: {}", worker_id);

    loop {
        let events = match persistence
            .outbox
            .claim_batch(&worker_id, config.batch_size, config.lease_timeout)
            .await
        {
            Ok(events) => events,
            Err(e) => {
                warn!("Failed to claim rule action outbox batch: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if events.is_empty() {
            sleep(config.idle_interval).await;
            continue;
        }

        let mut tasks = tokio::task::JoinSet::new();
        for event in events {
            while tasks.len() >= config.concurrency.max(1) {
                if let Some(Err(error)) = tasks.join_next().await {
                    warn!("Rule action outbox delivery task panicked: {error}");
                }
            }

            let persistence = persistence.clone();
            let cache = rule_cache.clone();
            let client = http_client.clone();
            let session = zenoh_session.clone();
            let metrics = zenoh_metrics.clone();
            let processing_worker_id = worker_id.clone();
            let span = tracing::info_span!(
                "rule_action_delivery",
                event_id = %event.id,
                tenant_id = %event.tenant_id,
                event_type = %event.event_type,
            );
            tasks.spawn(
                async move {
                    process_outbox_event(
                        event,
                        &processing_worker_id,
                        &persistence,
                        &cache,
                        &client,
                        &session,
                        &metrics,
                    )
                    .await;
                }
                .instrument(span),
            );
        }
        while let Some(result) = tasks.join_next().await {
            if let Err(error) = result {
                warn!("Rule action outbox delivery task panicked: {error}");
            }
        }
    }
}

async fn process_outbox_event(
    event: OutboxEventRecord,
    worker_id: &str,
    persistence: &Persistence,
    rule_cache: &Arc<RwLock<RuleCache>>,
    http_client: &reqwest::Client,
    zenoh_session: &Arc<zenoh::Session>,
    zenoh_metrics: &Arc<ZenohMetrics>,
) {
    let result = match serde_json::from_value::<PendingAction>(event.payload.clone()) {
        Ok(action) => {
            execute_action(
                action,
                persistence,
                rule_cache,
                http_client,
                zenoh_session,
                zenoh_metrics,
                &event.id,
            )
            .await
        }
        Err(e) => Err(format!("failed to deserialize pending action: {e}")),
    };

    let mark_result = match result {
        Ok(()) => {
            persistence
                .outbox
                .mark_succeeded(&event.id, worker_id)
                .await
        }
        Err(error) => {
            persistence
                .outbox
                .mark_failed(
                    &event.id,
                    worker_id,
                    event.attempts,
                    event.max_attempts,
                    &error,
                )
                .await
        }
    };
    if let Err(error) = mark_result {
        warn!(%error, "Failed to mark rule action outbox event");
    }
}

pub async fn execute_action(
    action: PendingAction,
    persistence: &Persistence,
    rule_cache: &Arc<RwLock<RuleCache>>,
    _http_client: &reqwest::Client,
    zenoh_session: &Arc<zenoh::Session>,
    zenoh_metrics: &Arc<ZenohMetrics>,
    delivery_id: &str,
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

            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            let result = persistence
                .alerts
                .create(
                    &tenant,
                    NewAlertRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        rule_id: Some(rule_id),
                        device_id,
                        severity,
                        message,
                        triggered_value,
                    },
                )
                .await
                .map_err(|error| error.to_string());

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
            tenant_id,
            alert_id,
            triggered_value,
        } => {
            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            if persistence
                .alerts
                .update_triggered_value(&tenant, &alert_id, triggered_value)
                .await
                .map_err(|error| error.to_string())?
            {
                Ok(())
            } else {
                Err(format!("Alert '{alert_id}' not found"))
            }
        }
        PendingAction::ResolveAlert {
            tenant_id,
            alert_id,
        } => {
            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            let result = persistence
                .alerts
                .transition(&tenant, &alert_id, AlertTransition::Resolve)
                .await
                .map_err(|error| error.to_string());

            match result {
                Ok(AlertTransitionOutcome::Updated(alert)) => {
                    if let Ok(mut cache) = rule_cache.write()
                        && let Some(rule_id) = &alert.rule_id
                    {
                        cache.active_alerts.remove(&(
                            alert.tenant_id.clone(),
                            rule_id.clone(),
                            alert.device_id.clone(),
                        ));
                    }
                    Ok(())
                }
                Ok(AlertTransitionOutcome::NotFound) => {
                    Err(format!("Alert '{alert_id}' not found"))
                }
                Ok(AlertTransitionOutcome::InvalidStatus(status)) => Err(format!(
                    "Alert '{alert_id}' cannot be resolved from status '{status}'"
                )),
                Err(error) => Err(error),
            }
        }
        PendingAction::SendWebhook {
            tenant_id: _,
            url,
            headers,
            payload,
        } => {
            let parsed_url = crate::security::validate_public_https_url(&url, "webhook url")
                .map_err(|e| e.to_string())?;
            let resolved_addrs =
                crate::security::validate_resolved_public_target(&parsed_url).await?;
            let host = parsed_url
                .host_str()
                .ok_or_else(|| "webhook url must include a host".to_string())?;
            let pinned_client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .resolve_to_addrs(host, &resolved_addrs)
                .build()
                .map_err(|e| e.to_string())?;
            let mut req = pinned_client.post(&url).json(&payload);
            for (key, value) in &headers {
                req = req.header(key, value);
            }
            req = req
                .header("Idempotency-Key", delivery_id)
                .header("X-Extrittio-Event-ID", delivery_id);
            let mut trace_headers = reqwest::header::HeaderMap::new();
            global::get_text_map_propagator(|propagator| {
                propagator.inject_context(
                    &tracing::Span::current().context(),
                    &mut HeaderInjector(&mut trace_headers),
                );
            });
            req = req.headers(trace_headers);
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

            let correlation_id = delivery_id.to_string();
            let params_json =
                serde_json::to_string(&params_map).unwrap_or_else(|_| "{}".to_string());
            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            persistence
                .commands
                .create(
                    &tenant,
                    &device_id,
                    NewCommandRecord {
                        id: correlation_id.clone(),
                        command: command.clone(),
                        params: params_json,
                    },
                )
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("Device '{device_id}' not found"))?;

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
            persistence
                .alerts
                .persist_cooldowns(vec![CooldownRecord {
                    tenant_id: tenant_id.clone(),
                    rule_id: rule_id.clone(),
                    device_id: device_id.clone(),
                    last_fired_at: fired_at,
                }])
                .await
                .map_err(|error| error.to_string())?;

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

struct HeaderInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) else {
            return;
        };
        let Ok(value) = reqwest::header::HeaderValue::from_str(&value) else {
            return;
        };
        self.0.insert(name, value);
    }
}

fn tenant_id_for_action(action: &PendingAction) -> &str {
    match action {
        PendingAction::CreateAlert { tenant_id, .. }
        | PendingAction::UpdateAlertValue { tenant_id, .. }
        | PendingAction::ResolveAlert { tenant_id, .. }
        | PendingAction::SendWebhook { tenant_id, .. }
        | PendingAction::SendCommand { tenant_id, .. }
        | PendingAction::UpdateCooldown { tenant_id, .. }
        | PendingAction::UpdateZoneEntry { tenant_id, .. } => tenant_id,
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
        | PendingAction::ResolveAlert { alert_id, .. } => alert_id.clone(),
        PendingAction::SendWebhook { url, .. } => url.clone(),
        PendingAction::SendCommand {
            device_id, command, ..
        } => format!("{device_id}:{command}"),
    }
}

fn idempotency_key_for_action(action: &PendingAction) -> String {
    match action {
        PendingAction::CreateAlert {
            rule_id, device_id, ..
        } => format!("create-alert:{rule_id}:{device_id}"),
        PendingAction::UpdateAlertValue {
            alert_id,
            triggered_value,
            ..
        } => format!(
            "update-alert-value:{alert_id}:{}",
            stable_hash(triggered_value.as_bytes())
        ),
        PendingAction::ResolveAlert { alert_id, .. } => format!("resolve-alert:{alert_id}"),
        PendingAction::SendWebhook {
            url,
            headers,
            payload,
            ..
        } => format!(
            "webhook:{url}:{}:{}",
            stable_json_hash(headers),
            stable_json_hash(payload)
        ),
        PendingAction::SendCommand {
            device_id,
            command,
            params,
            ..
        } => format!(
            "send-command:{device_id}:{command}:{}",
            stable_json_hash(params)
        ),
        PendingAction::UpdateCooldown {
            rule_id, device_id, ..
        } => format!("update-cooldown:{rule_id}:{device_id}"),
        PendingAction::UpdateZoneEntry {
            rule_id, device_id, ..
        } => format!("update-zone-entry:{rule_id}:{device_id}"),
    }
}

fn stable_json_hash<T>(value: &T) -> String
where
    T: serde::Serialize,
{
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    stable_hash(&bytes)
}

fn stable_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}
