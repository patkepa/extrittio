use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "otlp")]
use opentelemetry::global;
#[cfg(feature = "otlp")]
use opentelemetry::propagation::Injector;
use tokio::time::sleep;
use tracing::{Instrument, info, warn};
#[cfg(feature = "otlp")]
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::types::PendingAction;
use crate::persistence::RepositorySet;
use crate::state::ZenohMetrics;
use crate::tenancy::TenantId;
use extrittio_backend_core::alerts::CooldownRecord;
use extrittio_backend_core::outbox::OutboxEventRecord;

#[derive(Debug, Clone, Copy)]
pub struct OutboxWorkerConfig {
    pub batch_size: i64,
    pub concurrency: usize,
    pub idle_interval: Duration,
    pub lease_timeout: Duration,
}

pub async fn run_rule_action_outbox_worker(
    persistence: RepositorySet,
    http_client: reqwest::Client,
    zenoh_session: Arc<zenoh::Session>,
    zenoh_metrics: Arc<ZenohMetrics>,
    config: OutboxWorkerConfig,
) {
    let outbox = extrittio_backend_core::OutboxWorkerApplication::new(persistence.outbox.clone());
    let worker_id = format!("rule-action-worker-{}", uuid::Uuid::new_v4());
    info!("Rule action outbox worker started: {}", worker_id);

    loop {
        let events = match outbox
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
            let outbox = outbox.clone();
            let client = http_client.clone();
            let session = zenoh_session.clone();
            let metrics = zenoh_metrics.clone();
            let span = tracing::info_span!(
                "rule_action_delivery",
                event_id = %event.id,
                tenant_id = %event.tenant_id,
                event_type = %event.event_type,
            );
            tasks.spawn(
                async move {
                    process_outbox_event(event, &outbox, &persistence, &client, &session, &metrics)
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
    outbox: &extrittio_backend_core::OutboxWorkerApplication,
    persistence: &RepositorySet,
    http_client: &reqwest::Client,
    zenoh_session: &Arc<zenoh::Session>,
    zenoh_metrics: &Arc<ZenohMetrics>,
) {
    let mut failure = extrittio_backend_core::outbox::FailureClass::Retryable;
    let result = match extrittio_backend_core::rule_actions::decode_event(&event) {
        Ok(action) => {
            execute_action(
                action,
                persistence,
                http_client,
                zenoh_session,
                zenoh_metrics,
                &event.id,
                event.created_at,
            )
            .await
        }
        Err(e) => {
            failure = extrittio_backend_core::outbox::FailureClass::Permanent;
            Err(format!("failed to deserialize pending action: {e}"))
        }
    };

    let mark_result = match result {
        Ok(()) => outbox.succeeded(&event).await,
        Err(error) => outbox.failed(&event, failure, &error).await,
    };
    match mark_result {
        Ok(true) => {}
        Ok(false) => {
            tracing::debug!(event_id = %event.id, "Ignoring completion of a superseded outbox claim")
        }
        Err(error) => warn!(%error, "Failed to mark rule action outbox event"),
    }
}

pub async fn execute_action(
    action: PendingAction,
    persistence: &RepositorySet,
    _http_client: &reqwest::Client,
    zenoh_session: &Arc<zenoh::Session>,
    zenoh_metrics: &Arc<ZenohMetrics>,
    delivery_id: &str,
    delivery_created_at: chrono::NaiveDateTime,
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
            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            extrittio_backend_core::AlertWorkerApplication::new(persistence.alerts.clone())
                .create_for_action(
                    &tenant,
                    delivery_id,
                    extrittio_backend_core::RuleAlertIntent {
                        rule_id,
                        device_id,
                        severity,
                        message,
                        triggered_value,
                    },
                )
                .await
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        PendingAction::UpdateAlertValue {
            tenant_id,
            alert_id,
            triggered_value,
        } => {
            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            extrittio_backend_core::AlertWorkerApplication::new(persistence.alerts.clone())
                .update_value_for_action(&tenant, &alert_id, triggered_value)
                .await
                .map_err(|e| e.to_string())
        }
        PendingAction::ResolveAlert {
            tenant_id,
            alert_id,
        } => {
            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            extrittio_backend_core::AlertWorkerApplication::new(persistence.alerts.clone())
                .resolve_for_action(&tenant, &alert_id)
                .await
                .map_err(|e| e.to_string())
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
            #[cfg(feature = "otlp")]
            {
                let mut trace_headers = reqwest::header::HeaderMap::new();
                global::get_text_map_propagator(|propagator| {
                    propagator.inject_context(
                        &tracing::Span::current().context(),
                        &mut HeaderInjector(&mut trace_headers),
                    );
                });
                req = req.headers(trace_headers);
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
            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            let bus = Arc::new(crate::outbound::device_bus::ZenohDeviceBus::new(
                zenoh_session.clone(),
                zenoh_metrics.clone(),
            ));
            extrittio_backend_core::CommandApplication::new(
                persistence.commands.clone(),
                persistence.devices.clone(),
                bus,
            )
            .deliver_action(&tenant, &device_id, delivery_id, &command, params)
            .await
            .map_err(|error| error.to_string())?;
            Ok(())
        }
        PendingAction::UpdateCooldown {
            tenant_id,
            rule_id,
            device_id,
            fired_at,
        } => {
            extrittio_backend_core::AlertWorkerApplication::new(persistence.alerts.clone())
                .apply_legacy_cooldown(CooldownRecord {
                    tenant_id,
                    rule_id,
                    device_id,
                    last_fired_at: fired_at,
                })
                .await
                .map_err(|error| error.to_string())?;

            Ok(())
        }
        PendingAction::UpdateZoneEntry {
            tenant_id,
            rule_id,
            device_id,
            entered_at,
        } => {
            let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
            extrittio_backend_core::RuleRuntimeApplication::new(persistence.rules.clone())
                .apply_legacy_zone_entry(
                    &tenant,
                    extrittio_backend_core::rule_snapshots::LegacyZoneEntry {
                        rule_id,
                        device_id,
                        entered_at,
                        event_id: delivery_id.to_owned(),
                        created_at: delivery_created_at,
                    },
                )
                .await
                .map_err(|error| error.to_string())
        }
    }
}

#[cfg(feature = "otlp")]
struct HeaderInjector<'a>(&'a mut reqwest::header::HeaderMap);

#[cfg(feature = "otlp")]
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
