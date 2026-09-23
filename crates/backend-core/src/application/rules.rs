use serde_json::Value;
use uuid::Uuid;

use super::require_permission;
use crate::device_blueprints::DeviceBlueprintRepository;
use crate::rules::RuleRepository;
use crate::rules::WebhookUrlPolicy;
use crate::rules::{
    NewRuleRecord, RuleActionRecord, RuleConditionInput, RuleConditionRecord, RuleDetails,
    RuleFilter, UpdateRuleRecord,
};
use crate::zones::ZoneRepository;
use crate::{ApplicationError, Clock, Permission, TenantContext};
use std::sync::Arc;

const TELEMETRY_OPERATORS: &[&str] = &["gt", "gte", "lt", "lte", "eq", "neq"];
const STATUS_VALUES: &[&str] = &["online", "offline", "warning"];
const TARGET_TYPES: &[&str] = &["global", "blueprint", "fleet", "device"];
const TRIGGER_TYPES: &[&str] = &["telemetry", "device_status", "geofence"];

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
                &[RuleConditionInput {
                    field: field.into(),
                    blueprint_id: Some("blueprint-a".into()),
                    blueprint_revision_id: Some("revision-a".into()),
                    operator: "gt".into(),
                    value: threshold.into(),
                    zone_id: None,
                }],
                &[("alert".into(), serde_json::json!({}))],
            )
        };
        assert!(validate("machine.v2./counter/total", "9007199254740993").is_ok());
        assert!(validate("temperature", "20").is_err());
        assert!(validate("machine./counter", "NaN").is_err());
        assert!(validate("machine./counter", "inf").is_err());
    }

    #[test]
    fn geofence_requires_one_state_and_only_inside_dwell() {
        let state = RuleConditionInput {
            field: "zone_state".into(),
            blueprint_id: None,
            blueprint_revision_id: None,
            operator: "eq".into(),
            value: "inside".into(),
            zone_id: Some("zone-a".into()),
        };
        let dwell = RuleConditionInput {
            field: "dwell_seconds".into(),
            blueprint_id: None,
            blueprint_revision_id: None,
            operator: "gte".into(),
            value: "30".into(),
            zone_id: Some("zone-a".into()),
        };
        let validate = |conditions: &[RuleConditionInput]| {
            validate_rule(
                &NoWebhooks,
                "zone rule",
                "geofence",
                "global",
                &None,
                0,
                conditions,
                &[("alert".into(), serde_json::json!({}))],
            )
        };
        assert!(validate(&[state.clone(), dwell.clone()]).is_ok());
        assert!(validate(&[state.clone(), state.clone()]).is_err());
        assert!(validate(std::slice::from_ref(&dwell)).is_err());
        assert!(
            validate(&[
                RuleConditionInput {
                    value: "outside".into(),
                    ..state.clone()
                },
                dwell
            ])
            .is_err()
        );
    }
}

fn condition_records(conditions: Vec<RuleConditionInput>) -> Vec<RuleConditionRecord> {
    conditions
        .into_iter()
        .map(|condition| RuleConditionRecord {
            id: Uuid::new_v4().to_string(),
            field: condition.field,
            blueprint_id: condition.blueprint_id,
            blueprint_revision_id: condition.blueprint_revision_id,
            operator: condition.operator,
            value: condition.value,
            condition_group: 0,
            zone_id: condition.zone_id,
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
    blueprints: Arc<dyn DeviceBlueprintRepository>,
    zones: Arc<dyn ZoneRepository>,
    clock: Arc<dyn Clock>,
    webhook_urls: Arc<dyn WebhookUrlPolicy>,
    changes: Arc<dyn crate::rules::RuleChangeNotifier>,
}
impl RuleApplication {
    pub fn new(
        repository: Arc<dyn RuleRepository>,
        blueprints: Arc<dyn DeviceBlueprintRepository>,
        zones: Arc<dyn ZoneRepository>,
        clock: Arc<dyn Clock>,
        webhook_urls: Arc<dyn WebhookUrlPolicy>,
        changes: Arc<dyn crate::rules::RuleChangeNotifier>,
    ) -> Self {
        Self {
            repository,
            blueprints,
            zones,
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
        conditions: Vec<RuleConditionInput>,
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
        self.validate_metric_declarations(
            ctx,
            trigger_type,
            target_type,
            target_id.as_deref(),
            &conditions,
        )
        .await?;
        self.validate_geofence_zones(ctx, trigger_type, &conditions)
            .await?;
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
        conditions: Option<Vec<RuleConditionInput>>,
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
                .map(|condition| RuleConditionInput {
                    field: condition.field.clone(),
                    blueprint_id: condition.blueprint_id.clone(),
                    blueprint_revision_id: condition.blueprint_revision_id.clone(),
                    operator: condition.operator.clone(),
                    value: condition.value.clone(),
                    zone_id: condition.zone_id.clone(),
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
        self.validate_metric_declarations(
            ctx,
            trigger_type
                .as_deref()
                .unwrap_or(&current.rule.trigger_type),
            target_type.as_deref().unwrap_or(&current.rule.target_type),
            resolved_target_id.as_deref(),
            &validated_conditions,
        )
        .await?;
        self.validate_geofence_zones(
            ctx,
            trigger_type
                .as_deref()
                .unwrap_or(&current.rule.trigger_type),
            &validated_conditions,
        )
        .await?;
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

    async fn validate_metric_declarations(
        &self,
        ctx: &TenantContext,
        trigger_type: &str,
        target_type: &str,
        target_id: Option<&str>,
        conditions: &[RuleConditionInput],
    ) -> Result<(), ApplicationError> {
        if trigger_type != "telemetry" {
            return Ok(());
        }
        let mut shared_revision = None;
        for condition in conditions {
            let blueprint_id = condition.blueprint_id.as_deref().ok_or_else(|| {
                ApplicationError::InvalidInput("telemetry selector requires blueprint_id".into())
            })?;
            let revision_id = condition.blueprint_revision_id.as_deref().ok_or_else(|| {
                ApplicationError::InvalidInput(
                    "telemetry selector requires blueprint_revision_id".into(),
                )
            })?;
            if target_type == "blueprint" && target_id != Some(blueprint_id) {
                return Err(ApplicationError::InvalidInput(
                    "telemetry selector blueprint must match the rule target".into(),
                ));
            }
            if shared_revision.is_some_and(|previous| previous != revision_id) {
                return Err(ApplicationError::InvalidInput(
                    "all telemetry conditions must use one blueprint revision".into(),
                ));
            }
            shared_revision = Some(revision_id);
            let revision = self
                .blueprints
                .get_revision(ctx.tenant_id(), revision_id)
                .await?
                .filter(|revision| revision.blueprint_id == blueprint_id)
                .ok_or_else(|| {
                    ApplicationError::InvalidInput(
                        "telemetry selector references an unknown blueprint revision".into(),
                    )
                })?;
            let blueprint: extrittio_device_contract::DeviceBlueprint =
                serde_json::from_value(revision.document).map_err(|error| {
                    ApplicationError::Internal(format!(
                        "Stored published blueprint is invalid: {error}"
                    ))
                })?;
            let selector = crate::rule_engine::metric::MetricSelector::parse(&condition.field)
                .ok_or_else(|| {
                    ApplicationError::InvalidInput("invalid telemetry selector path".into())
                })?;
            let field = blueprint
                .spec
                .streams
                .iter()
                .find(|stream| stream.key == selector.stream_key)
                .and_then(|stream| {
                    stream
                        .fields
                        .iter()
                        .find(|field| field.path == selector.field_path)
                })
                .filter(|field| field.value_type.is_numeric())
                .ok_or_else(|| {
                    ApplicationError::InvalidInput(
                        "telemetry selector is not a declared numeric field".into(),
                    )
                })?;
            if field.value_type == extrittio_device_contract::FieldValueType::Int64
                && condition.value.parse::<i64>().is_err()
            {
                return Err(ApplicationError::InvalidInput(
                    "int64 telemetry conditions require an exact integer threshold".into(),
                ));
            }
        }
        Ok(())
    }

    async fn validate_geofence_zones(
        &self,
        ctx: &TenantContext,
        trigger_type: &str,
        conditions: &[RuleConditionInput],
    ) -> Result<(), ApplicationError> {
        if trigger_type != "geofence" {
            return Ok(());
        }
        let zone_id = conditions[0]
            .zone_id
            .as_deref()
            .expect("geofence conditions validated");
        if self.zones.get(ctx.tenant_id(), zone_id).await?.is_none() {
            return Err(ApplicationError::InvalidInput(
                "geofence zone is unknown".into(),
            ));
        }
        Ok(())
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "validation mirrors the persisted rule fields"
)]
fn validate_rule(
    webhook_urls: &dyn WebhookUrlPolicy,
    name: &str,
    trigger_type: &str,
    target_type: &str,
    target_id: &Option<String>,
    cooldown_seconds: i32,
    conditions: &[RuleConditionInput],
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
    for condition in conditions {
        let field = &condition.field;
        let operator = &condition.operator;
        let value = &condition.value;
        if trigger_type == "telemetry" {
            if crate::rule_engine::metric::MetricSelector::parse(field).is_none()
                || condition.blueprint_id.as_deref().is_none_or(str::is_empty)
                || condition
                    .blueprint_revision_id
                    .as_deref()
                    .is_none_or(str::is_empty)
                || condition.zone_id.is_some()
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
        } else if trigger_type == "geofence" {
            if condition.blueprint_id.is_some()
                || condition.blueprint_revision_id.is_some()
                || condition.zone_id.as_deref().is_none_or(str::is_empty)
                || !match field.as_str() {
                    "zone_state" => {
                        operator == "eq" && ["inside", "outside"].contains(&value.as_str())
                    }
                    "dwell_seconds" => {
                        operator == "gte" && value.parse::<i64>().is_ok_and(|seconds| seconds >= 0)
                    }
                    _ => false,
                }
            {
                return Err(ApplicationError::InvalidInput(
                    "invalid geofence condition".into(),
                ));
            }
        } else if field != "status"
            || condition.blueprint_id.is_some()
            || condition.blueprint_revision_id.is_some()
            || condition.zone_id.is_some()
            || !["eq", "neq"].contains(&operator.as_str())
            || !STATUS_VALUES.contains(&value.as_str())
        {
            return Err(ApplicationError::InvalidInput(
                "invalid device_status condition".into(),
            ));
        }
    }
    if trigger_type == "geofence" {
        let zone_id = conditions[0].zone_id.as_deref();
        let states: Vec<_> = conditions
            .iter()
            .filter(|condition| condition.field == "zone_state")
            .collect();
        let dwells: Vec<_> = conditions
            .iter()
            .filter(|condition| condition.field == "dwell_seconds")
            .collect();
        if states.len() != 1
            || dwells.len() > 1
            || conditions.len() != states.len() + dwells.len()
            || conditions
                .iter()
                .any(|condition| condition.zone_id.as_deref() != zone_id)
            || (!dwells.is_empty() && states[0].value != "inside")
        {
            return Err(ApplicationError::InvalidInput(
                "geofence requires one zone state and an optional inside dwell threshold".into(),
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
