use super::{AlertWorkerApplication, CommandApplication, OutboxWorkerApplication};
use crate::outbox::{FailureClass, OutboxEventRecord};
use crate::rule_engine::types::PendingAction;
use crate::{ApplicationError, TenantId};
use std::sync::Arc;

#[derive(Clone)]
pub struct RuleDeliveryApplication {
    outbox: OutboxWorkerApplication,
    alerts: AlertWorkerApplication,
    commands: CommandApplication,
    webhook: Arc<dyn crate::rule_actions::WebhookSender>,
}
impl RuleDeliveryApplication {
    pub fn new(
        outbox: OutboxWorkerApplication,
        alerts: AlertWorkerApplication,
        commands: CommandApplication,
        webhook: Arc<dyn crate::rule_actions::WebhookSender>,
    ) -> Self {
        Self {
            outbox,
            alerts,
            commands,
            webhook,
        }
    }
    pub async fn deliver(&self, event: &OutboxEventRecord) -> Result<bool, ApplicationError> {
        let (result, failure) = match crate::rule_actions::decode_event(event) {
            Ok(action) => (
                self.execute(action, &event.id).await,
                FailureClass::Retryable,
            ),
            Err(error) => (
                Err(format!("failed to deserialize pending action: {error}")),
                FailureClass::Permanent,
            ),
        };
        match result {
            Ok(()) => self.outbox.succeeded(event).await,
            Err(error) => self.outbox.failed(event, failure, &error).await,
        }
    }
    async fn execute(&self, action: PendingAction, delivery_id: &str) -> Result<(), String> {
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
                self.alerts
                    .create_for_action(
                        &tenant,
                        delivery_id,
                        crate::RuleAlertIntent {
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
                self.alerts
                    .update_value_for_action(&tenant, &alert_id, triggered_value)
                    .await
                    .map_err(|e| e.to_string())
            }
            PendingAction::ResolveAlert {
                tenant_id,
                alert_id,
            } => {
                let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
                self.alerts
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
                self.webhook
                    .send(&url, &headers, &payload, delivery_id)
                    .await
            }
            PendingAction::SendCommand {
                tenant_id,
                device_id,
                command,
                params,
            } => {
                let tenant = TenantId::new(tenant_id).map_err(|error| error.to_string())?;
                self.commands
                    .deliver_action(&tenant, &device_id, delivery_id, &command, params)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(())
            }
            PendingAction::UpdateCooldown { .. } | PendingAction::UpdateZoneEntry { .. } => {
                Err("runtime state must be committed during ingestion, not delivered".into())
            }
        }
    }
}
