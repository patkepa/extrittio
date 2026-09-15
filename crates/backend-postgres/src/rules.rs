use async_trait::async_trait;
use chrono::NaiveDateTime;
use diesel::{Connection, OptionalExtension, PgConnection};

use crate::models::{
    NewRule, NewRuleAction, NewRuleCondition, Rule, RuleAction, RuleCondition, UpdateRule,
};
use crate::rules_sql as rule_repo;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::rule_engine::types::{CachedAction, CachedCondition, CachedRule};
use extrittio_backend_core::rule_snapshots::RuleSnapshotRecords;
use extrittio_backend_core::rules::RuleRepository;
use extrittio_backend_core::rules::{
    NewRuleRecord, RuleActionRecord, RuleConditionRecord, RuleDetails, RuleFilter, RuleRecord,
    UpdateRuleRecord,
};

use crate::error::map_diesel_error;
use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresRuleRepository {
    executor: PostgresExecutor,
}
impl PostgresRuleRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
#[derive(Debug, thiserror::Error)]
enum SnapshotReadError {
    #[error(transparent)]
    Diesel(#[from] diesel::result::Error),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

fn rule_record(rule: Rule) -> RuleRecord {
    RuleRecord {
        id: rule.id,
        tenant_id: rule.tenant_id,
        name: rule.name,
        description: rule.description,
        enabled: rule.enabled,
        trigger_type: rule.trigger_type,
        target_type: rule.target_type,
        target_id: rule.target_id,
        cooldown_seconds: rule.cooldown_seconds,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
    }
}

fn condition_record(condition: RuleCondition) -> RuleConditionRecord {
    RuleConditionRecord {
        id: condition.id,
        field: condition.field,
        operator: condition.operator,
        value: condition.value,
        condition_group: condition.condition_group,
        zone_id: condition.zone_id,
    }
}

fn action_record(action: RuleAction) -> RuleActionRecord {
    RuleActionRecord {
        id: action.id,
        action_type: action.action_type,
        config: action.config,
    }
}

fn details(rule: Rule, conditions: Vec<RuleCondition>, actions: Vec<RuleAction>) -> RuleDetails {
    RuleDetails {
        rule: rule_record(rule),
        conditions: conditions.into_iter().map(condition_record).collect(),
        actions: actions.into_iter().map(action_record).collect(),
    }
}

fn load_details(
    connection: &mut PgConnection,
    tenant_id: &str,
    id: &str,
) -> Result<Option<RuleDetails>, diesel::result::Error> {
    let Some(rule) = rule_repo::find_rule(connection, tenant_id, id).optional()? else {
        return Ok(None);
    };
    let conditions = rule_repo::list_conditions(connection, tenant_id, id)?;
    let actions = rule_repo::list_actions(connection, tenant_id, id)?;
    Ok(Some(details(rule, conditions, actions)))
}

#[async_trait]
impl RuleRepository for PostgresRuleRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        filter: RuleFilter,
    ) -> Result<Vec<RuleDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                rule_repo::load_rules_with_details(
                    connection,
                    &tenant_id,
                    filter.enabled,
                    filter.trigger_type.as_deref(),
                    filter.target_type.as_deref(),
                )
                .map(|rows| {
                    rows.into_iter()
                        .map(|(rule, conditions, actions)| details(rule, conditions, actions))
                        .collect()
                })
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn get(
        &self,
        tenant: &TenantId,
        id: &str,
    ) -> Result<Option<RuleDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let id = id.to_string();
        self.executor
            .run(move |connection| {
                load_details(connection, &tenant_id, &id).map_err(map_diesel_error)
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: NewRuleRecord,
    ) -> Result<RuleDetails, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        rule_repo::insert_rule(
                            connection,
                            &NewRule {
                                id: record.id.clone(),
                                tenant_id: tenant_id.clone(),
                                name: record.name,
                                description: record.description,
                                enabled: true,
                                trigger_type: record.trigger_type,
                                target_type: record.target_type,
                                target_id: record.target_id,
                                cooldown_seconds: record.cooldown_seconds,
                            },
                        )?;
                        let conditions: Vec<NewRuleCondition> = record
                            .conditions
                            .into_iter()
                            .map(|condition| NewRuleCondition {
                                id: condition.id,
                                tenant_id: tenant_id.clone(),
                                rule_id: record.id.clone(),
                                field: condition.field,
                                operator: condition.operator,
                                value: condition.value,
                                condition_group: condition.condition_group,
                                zone_id: condition.zone_id,
                            })
                            .collect();
                        let actions: Vec<NewRuleAction> = record
                            .actions
                            .into_iter()
                            .map(|action| NewRuleAction {
                                id: action.id,
                                tenant_id: tenant_id.clone(),
                                rule_id: record.id.clone(),
                                action_type: action.action_type,
                                config: action.config,
                            })
                            .collect();
                        rule_repo::insert_conditions(connection, &conditions)?;
                        rule_repo::insert_actions(connection, &actions)?;
                        load_details(connection, &tenant_id, &record.id)?
                            .ok_or(diesel::result::Error::NotFound)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn update(
        &self,
        tenant: &TenantId,
        id: &str,
        record: UpdateRuleRecord,
    ) -> Result<Option<RuleDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let id = id.to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        if rule_repo::find_rule(connection, &tenant_id, &id)
                            .optional()?
                            .is_none()
                        {
                            return Ok(None);
                        }
                        rule_repo::update_rule(
                            connection,
                            &tenant_id,
                            &id,
                            &UpdateRule {
                                name: record.name,
                                description: record.description,
                                enabled: None,
                                trigger_type: record.trigger_type,
                                target_type: record.target_type,
                                target_id: record.target_id,
                                cooldown_seconds: record.cooldown_seconds,
                                updated_at: Some(record.updated_at),
                            },
                        )?;
                        if let Some(conditions) = record.conditions {
                            rule_repo::delete_conditions_for_rule(connection, &tenant_id, &id)?;
                            let conditions: Vec<NewRuleCondition> = conditions
                                .into_iter()
                                .map(|condition| NewRuleCondition {
                                    id: condition.id,
                                    tenant_id: tenant_id.clone(),
                                    rule_id: id.clone(),
                                    field: condition.field,
                                    operator: condition.operator,
                                    value: condition.value,
                                    condition_group: condition.condition_group,
                                    zone_id: condition.zone_id,
                                })
                                .collect();
                            if !conditions.is_empty() {
                                rule_repo::insert_conditions(connection, &conditions)?;
                            }
                        }
                        if let Some(actions) = record.actions {
                            rule_repo::delete_actions_for_rule(connection, &tenant_id, &id)?;
                            let actions: Vec<NewRuleAction> = actions
                                .into_iter()
                                .map(|action| NewRuleAction {
                                    id: action.id,
                                    tenant_id: tenant_id.clone(),
                                    rule_id: id.clone(),
                                    action_type: action.action_type,
                                    config: action.config,
                                })
                                .collect();
                            if !actions.is_empty() {
                                rule_repo::insert_actions(connection, &actions)?;
                            }
                        }
                        load_details(connection, &tenant_id, &id)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete(&self, tenant: &TenantId, id: &str) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let id = id.to_string();
        self.executor
            .run(move |connection| {
                rule_repo::delete_rule(connection, &tenant_id, &id)
                    .map(|rows| rows > 0)
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn toggle(
        &self,
        tenant: &TenantId,
        id: &str,
        enabled: bool,
        updated_at: NaiveDateTime,
    ) -> Result<Option<RuleDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let id = id.to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let rows = rule_repo::update_rule(
                            connection,
                            &tenant_id,
                            &id,
                            &UpdateRule {
                                enabled: Some(enabled),
                                updated_at: Some(updated_at),
                                ..Default::default()
                            },
                        )?;
                        if rows == 0 {
                            return Ok(None);
                        }
                        load_details(connection, &tenant_id, &id)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn load_snapshot(&self) -> Result<RuleSnapshotRecords, PersistenceError> {
        self.executor
            .run(move |connection| {
                connection
                    .build_transaction()
                    .read_only()
                    .repeatable_read()
                    .run(|connection| {
                        let enabled_rules = rule_repo::load_all_enabled_rules(connection)
                            .map_err(map_diesel_error)?;
                        let mut cache = RuleSnapshotRecords::default();
                        for (rule, conditions, actions) in enabled_rules {
                            cache.rules.push(CachedRule {
                                tenant_id: rule.tenant_id,
                                id: rule.id,
                                name: rule.name,
                                trigger_type: rule.trigger_type,
                                target_type: rule.target_type,
                                target_id: rule.target_id,
                                cooldown_seconds: rule.cooldown_seconds,
                                conditions: conditions
                                    .into_iter()
                                    .map(|condition| CachedCondition {
                                        field: condition.field,
                                        operator: condition.operator,
                                        value: condition.value,
                                        zone_id: condition.zone_id,
                                    })
                                    .collect(),
                                actions: actions
                                    .into_iter()
                                    .map(|action| CachedAction {
                                        action_type: action.action_type,
                                        config: action.config,
                                    })
                                    .collect(),
                            });
                        }
                        let zones = crate::zones::list_snapshot_on_connection(connection)?;
                        extrittio_backend_core::rules::merge_zone_snapshots(&mut cache, zones);
                        Ok::<_, SnapshotReadError>(cache)
                    })
                    .map_err(|error| match error {
                        SnapshotReadError::Diesel(error) => map_diesel_error(error),
                        SnapshotReadError::Persistence(error) => error,
                    })
            })
            .await
    }

    async fn delete_stale_cooldowns(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        self.executor
            .run(move |connection| {
                rule_repo::delete_cooldowns_older_than(connection, cutoff).map_err(map_diesel_error)
            })
            .await
    }
}
