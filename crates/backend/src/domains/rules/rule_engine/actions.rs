use std::time::Duration;
use tokio::time::sleep;
use tracing::{Instrument, info, warn};

#[derive(Debug, Clone, Copy)]
pub struct OutboxWorkerConfig {
    pub batch_size: i64,
    pub concurrency: usize,
    pub idle_interval: Duration,
    pub lease_timeout: Duration,
}

pub async fn run_rule_action_outbox_worker(
    outbox: extrittio_backend_core::OutboxWorkerApplication,
    delivery: extrittio_backend_core::application::RuleDeliveryApplication,
    config: OutboxWorkerConfig,
) {
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

            let delivery = delivery.clone();
            let span = tracing::info_span!(
                "rule_action_delivery",
                event_id = %event.id,
                tenant_id = %event.tenant_id,
                event_type = %event.event_type,
            );
            tasks.spawn(
                async move {
                    match delivery.deliver(&event).await {
                        Ok(true) => {}
                        Ok(false) => tracing::debug!(event_id = %event.id, "Ignoring completion of a superseded outbox claim"),
                        Err(error) => warn!(%error, "Failed to mark rule action outbox event"),
                    }
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
