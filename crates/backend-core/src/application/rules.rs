use serde_json::Value;
use uuid::Uuid;

use super::require_permission;
use crate::rules::RuleRepository;
use crate::rules::WebhookUrlPolicy;
use crate::rules::{
    NewRuleRecord, RuleActionRecord, RuleConditionRecord, RuleDetails, RuleFilter, UpdateRuleRecord,
};
use crate::{ApplicationError, Clock, Permission, TenantContext};
use std::sync::Arc;

const TELEMETRY_OPERATORS: &[&str] = &["gt", "gte", "lt", "lte", "eq", "neq"];
const STATUS_VALUES: &[&str] = &["online", "offline", "warning"];
const TARGET_TYPES: &[&str] = &["global", "blueprint", "fleet", "device"];
const TRIGGER_TYPES: &[&str] = &["telemetry", "device_status"];

#[cfg(test)]
mod metric_validation_tests {
    use super::*;
    struct NoWebhooks;
    impl WebhookUrlPolicy for NoWebhooks {
        fn validate(&self, _: &str) -> Result<(), ApplicationError> {
            panic!("no webhook action")
        }
    }

    #[test]
    fn validates_arbitrary_stream_fields_and_rejects_fixed_names_or_nonfinite_values() {
        let validate = |field: &str, threshold: &str| {
            validate_rule(
                &NoWebhooks,
                "counter rule",
                "telemetry",
                "blueprint",
                &Some("blueprint-a".into()),
                0,
                &[(field.into(), "gt".into(), threshold.into())],
                &[("alert".into(), serde_json::json!({}))],
            )
        };
        assert!(validate("machine.v2./counter/total", "9007199254740993").is_ok());
        assert!(validate("temperature", "20").is_err());
        assert!(validate("machine./counter", "NaN").is_err());
        assert!(validate("machine./counter", "inf").is_err());
    }
}

fn condition_records(conditions: Vec<(String, String, String)>) -> Vec<RuleConditionRecord> {
    conditions
        .into_iter()
        .map(|(field, operator, value)| RuleConditionRecord {
            id: Uuid::new_v4().to_string(),
            field,
            operator,
            value,
            condition_group: 0,
            zone_id: None,
        })
        .collect()
}

fn action_records(actions: Vec<(String, Value)>) -> Vec<RuleActionRecord> {
    actions
        .into_iter()
        .map(|(action_type, config)| RuleActionRecord {
            id: Uuid::new_v4().to_string(),
            action_type,
            config,
        })
        .collect()
}

#[derive(Clone)]
pub struct RuleApplication {
    repository: Arc<dyn RuleRepository>,
    clock: Arc<dyn Clock>,
    webhook_urls: Arc<dyn WebhookUrlPolicy>,
    changes: Arc<dyn crate::rules::RuleChangeNotifier>,
}
impl RuleApplication {
    pub fn new(
        repository: Arc<dyn RuleRepository>,
        clock: Arc<dyn Clock>,
        webhook_urls: Arc<dyn WebhookUrlPolicy>,
        changes: Arc<dyn crate::rules::RuleChangeNotifier>,
    ) -> Self {
        Self {
            repository,
            clock,
            webhook_urls,
            changes,
        }
    }

    pub async fn list(
        &self,
        ctx: &TenantContext,
        filter: RuleFilter,
    ) -> Result<Vec<RuleDetails>, ApplicationError> {
        require_permission(ctx, Permission::ReadRules)?;
        Ok(self.repository.list(ctx.tenant_id(), filter).await?)
    }

    pub async fn get(
        &self,
        ctx: &TenantContext,
        id: &str,
    ) -> Result<RuleDetails, ApplicationError> {
        require_permission(ctx, Permission::ReadRules)?;
        self.repository
            .get(ctx.tenant_id(), id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Rule '{id}' not found")))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        ctx: &TenantContext,
        name: &str,
        description: Option<String>,
        trigger_type: &str,
        target_type: &str,
        target_id: Option<String>,
        cooldown_seconds: i32,
        conditions: Vec<(String, String, String)>,
        actions: Vec<(String, Value)>,
    ) -> Result<RuleDetails, ApplicationError> {
        require_permission(ctx, Permission::ManageRules)?;
        validate_rule(
            self.webhook_urls.as_ref(),
            name,
            trigger_type,
            target_type,
            &target_id,
            cooldown_seconds,
            &conditions,
            &actions,
        )?;
        let created = self
            .repository
            .create(
                ctx.tenant_id(),
                NewRuleRecord {
                    id: Uuid::new_v4().to_string(),
                    name: name.trim().to_string(),
                    description,
                    trigger_type: trigger_type.to_string(),
                    target_type: target_type.to_string(),
                    target_id,
                    cooldown_seconds,
                    conditions: condition_records(conditions),
                    actions: action_records(actions),
                },
            )
            .await?;
        self.changes.committed();
        Ok(created)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        ctx: &TenantContext,
        id: &str,
        name: Option<String>,
        description: Option<Option<String>>,
        trigger_type: Option<String>,
        target_type: Option<String>,
        target_id: Option<Option<String>>,
        cooldown_seconds: Option<i32>,
        conditions: Option<Vec<(String, String, String)>>,
        actions: Option<Vec<(String, Value)>>,
    ) -> Result<RuleDetails, ApplicationError> {
        require_permission(ctx, Permission::ManageRules)?;
        let current = self
            .repository
            .get(ctx.tenant_id(), id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Rule '{id}' not found")))?;
        let resolved_target_id = target_id
            .as_ref()
            .cloned()
            .unwrap_or_else(|| current.rule.target_id.clone());
        let validated_conditions = conditions.clone().unwrap_or_else(|| {
            current
                .conditions
                .iter()
                .map(|condition| {
                    (
                        condition.field.clone(),
                        condition.operator.clone(),
                        condition.value.clone(),
                    )
                })
                .collect()
        });
        let validated_actions = actions.clone().unwrap_or_else(|| {
            current
                .actions
                .iter()
                .map(|action| (action.action_type.clone(), action.config.clone()))
                .collect()
        });
        validate_rule(
            self.webhook_urls.as_ref(),
            name.as_deref().unwrap_or(&current.rule.name),
            trigger_type
                .as_deref()
                .unwrap_or(&current.rule.trigger_type),
            target_type.as_deref().unwrap_or(&current.rule.target_type),
            &resolved_target_id,
            cooldown_seconds.unwrap_or(current.rule.cooldown_seconds),
            &validated_conditions,
            &validated_actions,
        )?;
        self.repository
            .update(
                ctx.tenant_id(),
                id,
                UpdateRuleRecord {
                    name: name.map(|name| name.trim().to_string()),
                    description,
                    trigger_type,
                    target_type,
                    target_id,
                    cooldown_seconds,
                    conditions: conditions.map(condition_records),
                    actions: actions.map(action_records),
                    updated_at: self.clock.now().naive_utc(),
                },
            )
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Rule '{id}' not found")))
            .inspect(|_| self.changes.committed())
    }

    pub async fn delete(&self, ctx: &TenantContext, id: &str) -> Result<(), ApplicationError> {
        require_permission(ctx, Permission::ManageRules)?;
        if self.repository.delete(ctx.tenant_id(), id).await? {
            self.changes.committed();
            Ok(())
        } else {
            Err(ApplicationError::NotFound(format!("Rule '{id}' not found")))
        }
    }

    pub async fn toggle(
        &self,
        ctx: &TenantContext,
        id: &str,
        enabled: bool,
    ) -> Result<RuleDetails, ApplicationError> {
        require_permission(ctx, Permission::ManageRules)?;
        self.repository
            .toggle(ctx.tenant_id(), id, enabled, self.clock.now().naive_utc())
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Rule '{id}' not found")))
            .inspect(|_| self.changes.committed())
    }
}

fn validate_rule(
    webhook_urls: &dyn WebhookUrlPolicy,
    name: &str,
    trigger_type: &str,
    target_type: &str,
    target_id: &Option<String>,
    cooldown_seconds: i32,
    conditions: &[(String, String, String)],
    actions: &[(String, Value)],
) -> Result<(), ApplicationError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 255 {
        return Err(ApplicationError::InvalidInput(
            "name must contain between 1 and 255 characters".into(),
        ));
    }
    if !TRIGGER_TYPES.contains(&trigger_type) {
        return Err(ApplicationError::InvalidInput(format!(
            "trigger_type must be one of: {}",
            TRIGGER_TYPES.join(", ")
        )));
    }
    if !TARGET_TYPES.contains(&target_type) {
        return Err(ApplicationError::InvalidInput(format!(
            "target_type must be one of: {}",
            TARGET_TYPES.join(", ")
        )));
    }
    if (target_type == "global" && target_id.is_some())
        || (target_type != "global" && target_id.as_deref().is_none_or(str::is_empty))
    {
        return Err(ApplicationError::InvalidInput(if target_type == "global" {
            "target_id must be null for global rules".into()
        } else {
            format!("target_id is required for target_type '{target_type}'")
        }));
    }
    if !(0..=86_400).contains(&cooldown_seconds) {
        return Err(ApplicationError::InvalidInput(
            "cooldown_seconds must be between 0 and 86400".into(),
        ));
    }
    if conditions.is_empty() {
        return Err(ApplicationError::InvalidInput(
            "at least one condition is required".into(),
        ));
    }
    for (field, operator, value) in conditions {
        if trigger_type == "telemetry" {
            if crate::rule_engine::metric::MetricSelector::parse(field).is_none()
                || !TELEMETRY_OPERATORS.contains(&operator.as_str())
            {
                return Err(ApplicationError::InvalidInput(
                    "invalid telemetry condition field or operator".into(),
                ));
            }
            crate::rule_engine::number::MetricNumber::parse(value).ok_or_else(|| {
                ApplicationError::InvalidInput(format!(
                    "telemetry condition value '{value}' is not a valid number"
                ))
            })?;
        } else if field != "status"
            || !["eq", "neq"].contains(&operator.as_str())
            || !STATUS_VALUES.contains(&value.as_str())
        {
            return Err(ApplicationError::InvalidInput(
                "invalid device_status condition".into(),
            ));
        }
    }
    if actions.is_empty() {
        return Err(ApplicationError::InvalidInput(
            "at least one action is required".into(),
        ));
    }
    for (action_type, config) in actions {
        match action_type.as_str() {
            "webhook" => {
                let url = config.get("url").and_then(Value::as_str).unwrap_or("");
                if url.is_empty() {
                    return Err(ApplicationError::InvalidInput(
                        "webhook action config must have a non-empty 'url'".into(),
                    ));
                }
                webhook_urls.validate(url)?;
            }
            "command" => {
                if config
                    .get("command")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    return Err(ApplicationError::InvalidInput(
                        "command action config must have a non-empty 'command'".into(),
                    ));
                }
            }
            "alert" => {}
            other => {
                return Err(ApplicationError::InvalidInput(format!(
                    "unknown action_type '{other}'"
                )));
            }
        }
    }
    Ok(())
}
