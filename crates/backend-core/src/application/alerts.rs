use super::require_permission;
use crate::alerts::{
    AlertListFilter, AlertRecord, AlertRepository, AlertTransition, AlertTransitionOutcome,
};
use crate::{ApplicationError, Permission, TenantContext};
use std::sync::Arc;
#[derive(Clone)]
pub struct AlertApplication {
    repository: Arc<dyn AlertRepository>,
}
impl AlertApplication {
    pub fn new(repository: Arc<dyn AlertRepository>) -> Self {
        Self { repository }
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        filter: AlertListFilter,
    ) -> Result<(Vec<AlertRecord>, i64), ApplicationError> {
        require_permission(ctx, Permission::ReadAlerts)?;
        Ok(self.repository.list(ctx.tenant_id(), filter).await?)
    }

    pub async fn get(
        &self,
        ctx: &TenantContext,
        id: &str,
    ) -> Result<AlertRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadAlerts)?;
        self.repository
            .get(ctx.tenant_id(), id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Alert '{id}' not found")))
    }

    pub async fn transition(
        &self,
        ctx: &TenantContext,
        id: &str,
        transition: AlertTransition,
    ) -> Result<AlertRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageAlerts)?;
        match self
            .repository
            .transition(ctx.tenant_id(), id, transition)
            .await?
        {
            AlertTransitionOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "Alert '{id}' not found"
            ))),
            AlertTransitionOutcome::InvalidStatus(status) => {
                let message = match transition {
                    AlertTransition::Acknowledge => {
                        format!("Alert '{id}' cannot be acknowledged from status '{status}'")
                    }
                    AlertTransition::Resolve => format!("Alert '{id}' is already resolved"),
                    AlertTransition::Reactivate => format!("Alert '{id}' is already active"),
                };
                Err(ApplicationError::InvalidInput(message))
            }
            AlertTransitionOutcome::ActiveConflict(existing_id) => {
                Err(ApplicationError::Conflict(format!(
                    "Alert '{id}' cannot be reactivated while alert '{existing_id}' is active or acknowledged for the same rule and device"
                )))
            }
            AlertTransitionOutcome::Updated(alert) => Ok(*alert),
        }
    }

    pub async fn transition_many(
        &self,
        ctx: &TenantContext,
        ids: Vec<String>,
        transition: AlertTransition,
    ) -> Result<Vec<AlertRecord>, ApplicationError> {
        require_permission(ctx, Permission::ManageAlerts)?;
        Ok(self
            .repository
            .transition_many(ctx.tenant_id(), ids, transition)
            .await?)
    }

    pub async fn summary(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<(String, String, i64)>, ApplicationError> {
        require_permission(ctx, Permission::ReadAlerts)?;
        Ok(self.repository.summary(ctx.tenant_id()).await?)
    }
}

/// System worker intent, separate from tenant-facing alert management.
pub struct RuleAlertIntent {
    pub rule_id: String,
    pub device_id: String,
    pub severity: String,
    pub message: String,
    pub triggered_value: Option<String>,
}
#[derive(Clone)]
pub struct AlertWorkerApplication {
    repository: Arc<dyn AlertRepository>,
}
impl AlertWorkerApplication {
    pub fn new(repository: Arc<dyn AlertRepository>) -> Self {
        Self { repository }
    }
    pub async fn update_value_for_action(
        &self,
        tenant: &crate::TenantId,
        id: &str,
        value: String,
    ) -> Result<(), ApplicationError> {
        if self
            .repository
            .update_triggered_value(tenant, id, value)
            .await?
        {
            Ok(())
        } else {
            Err(ApplicationError::NotFound(format!(
                "Alert '{id}' not found"
            )))
        }
    }
    pub async fn resolve_for_action(
        &self,
        tenant: &crate::TenantId,
        id: &str,
    ) -> Result<(), ApplicationError> {
        match self
            .repository
            .transition(tenant, id, AlertTransition::Resolve)
            .await?
        {
            AlertTransitionOutcome::Updated(_) => Ok(()),
            AlertTransitionOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "Alert '{id}' not found"
            ))),
            AlertTransitionOutcome::ActiveConflict(existing) => Err(ApplicationError::Conflict(
                format!("Active alert conflict: {existing}"),
            )),
            AlertTransitionOutcome::InvalidStatus(status) => {
                Err(ApplicationError::InvalidOperation(format!(
                    "Alert '{id}' cannot be resolved from status '{status}'"
                )))
            }
        }
    }
    pub async fn apply_legacy_cooldown(
        &self,
        record: crate::alerts::CooldownRecord,
    ) -> Result<(), ApplicationError> {
        Ok(self.repository.persist_cooldowns(vec![record]).await?)
    }
    pub async fn create_for_action(
        &self,
        tenant: &crate::TenantId,
        delivery_id: &str,
        intent: RuleAlertIntent,
    ) -> Result<Option<AlertRecord>, ApplicationError> {
        if delivery_id.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "alert creation requires a durable action ID".into(),
            ));
        }
        Ok(self
            .repository
            .create_or_get_active(
                tenant,
                crate::alerts::NewRuleAlertRecord {
                    id: delivery_id.to_owned(),
                    rule_id: intent.rule_id,
                    device_id: intent.device_id,
                    severity: intent.severity,
                    message: intent.message,
                    triggered_value: intent.triggered_value,
                },
            )
            .await?)
    }
}

/// System-scoped retention, separate from tenant-facing alert management.
pub struct AlertMaintenanceApplication {
    alerts: Arc<dyn AlertRepository>,
    rules: Arc<dyn crate::rules::RuleRepository>,
}
impl AlertMaintenanceApplication {
    pub fn new(
        alerts: Arc<dyn AlertRepository>,
        rules: Arc<dyn crate::rules::RuleRepository>,
    ) -> Self {
        Self { alerts, rules }
    }
    pub async fn prune(
        &self,
        retention_days: u64,
        now: chrono::NaiveDateTime,
    ) -> Result<(usize, usize), ApplicationError> {
        let age = i64::try_from(retention_days)
            .ok()
            .and_then(chrono::Duration::try_days)
            .ok_or_else(|| {
                ApplicationError::InvalidInput("alert retention period is out of range".into())
            })?;
        let cutoff = now.checked_sub_signed(age).ok_or_else(|| {
            ApplicationError::InvalidInput("alert retention cutoff is out of range".into())
        })?;
        let cooldown_cutoff = now
            .checked_sub_signed(chrono::Duration::hours(24))
            .ok_or_else(|| {
                ApplicationError::InvalidInput("cooldown retention cutoff is out of range".into())
            })?;
        // Preserve the existing independent cleanup operations. A later failure
        // does not undo an earlier delete; retries remain idempotent.
        let alerts = self.alerts.delete_all_resolved_before(cutoff).await?;
        let cooldowns = self.rules.delete_stale_cooldowns(cooldown_cutoff).await?;
        Ok((alerts, cooldowns))
    }
}
