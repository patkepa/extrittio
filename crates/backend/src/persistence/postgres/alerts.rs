use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use diesel::{Connection, OptionalExtension, PgConnection};

use crate::db::models::{Alert, RuleCooldown, UpdateAlert};
use crate::domains::alerts::port::AlertRepository;
use crate::domains::alerts::types::{
    AlertListFilter, AlertRecord, AlertTransition, AlertTransitionOutcome, CooldownRecord,
};
use crate::persistence::PersistenceError;
use crate::repositories::{alert_repo, rule_repo};
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

fn alert_record(alert: Alert) -> AlertRecord {
    AlertRecord {
        id: alert.id,
        tenant_id: alert.tenant_id,
        rule_id: alert.rule_id,
        device_id: alert.device_id,
        severity: alert.severity,
        status: alert.status,
        message: alert.message,
        triggered_value: alert.triggered_value,
        resolved_at: alert.resolved_at,
        acknowledged_at: alert.acknowledged_at,
        created_at: alert.created_at,
    }
}

fn transition_alert(
    connection: &mut PgConnection,
    tenant_id: &str,
    id: &str,
    transition: AlertTransition,
) -> Result<AlertTransitionOutcome, diesel::result::Error> {
    let Some(alert) = alert_repo::find_alert(connection, tenant_id, id).optional()? else {
        return Ok(AlertTransitionOutcome::NotFound);
    };
    let valid = match transition {
        AlertTransition::Acknowledge => alert.status == "active",
        AlertTransition::Resolve => alert.status != "resolved",
        AlertTransition::Reactivate => alert.status != "active",
    };
    if !valid {
        return Ok(AlertTransitionOutcome::InvalidStatus(alert.status));
    }
    let now = Utc::now().naive_utc();
    let changeset = match transition {
        AlertTransition::Acknowledge => UpdateAlert {
            status: Some("acknowledged".to_string()),
            acknowledged_at: Some(Some(now)),
            resolved_at: None,
        },
        AlertTransition::Resolve => UpdateAlert {
            status: Some("resolved".to_string()),
            acknowledged_at: None,
            resolved_at: Some(Some(now)),
        },
        AlertTransition::Reactivate => UpdateAlert {
            status: Some("active".to_string()),
            acknowledged_at: Some(None),
            resolved_at: Some(None),
        },
    };
    alert_repo::update_alert(connection, tenant_id, id, &changeset)?;
    alert_repo::find_alert(connection, tenant_id, id)
        .map(alert_record)
        .map(Box::new)
        .map(AlertTransitionOutcome::Updated)
}

#[async_trait]
impl AlertRepository for PostgresAdapter {
    async fn list(
        &self,
        tenant: &TenantId,
        filter: AlertListFilter,
    ) -> Result<(Vec<AlertRecord>, i64), PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                alert_repo::list_alerts(
                    connection,
                    &tenant_id,
                    alert_repo::AlertListFilter {
                        status: filter.status.as_deref(),
                        severity: filter.severity.as_deref(),
                        device_id: filter.device_id.as_deref(),
                        rule_id: filter.rule_id.as_deref(),
                        since: filter.since,
                        before: filter.before,
                        limit: filter.limit,
                        offset: filter.offset,
                    },
                )
                .map(|(alerts, total)| (alerts.into_iter().map(alert_record).collect(), total))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn get(
        &self,
        tenant: &TenantId,
        id: &str,
    ) -> Result<Option<AlertRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let id = id.to_string();
        self.executor
            .run(move |connection| {
                alert_repo::find_alert(connection, &tenant_id, &id)
                    .optional()
                    .map(|alert| alert.map(alert_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn transition(
        &self,
        tenant: &TenantId,
        id: &str,
        transition: AlertTransition,
    ) -> Result<AlertTransitionOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let id = id.to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        transition_alert(connection, &tenant_id, &id, transition)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn transition_many(
        &self,
        tenant: &TenantId,
        ids: Vec<String>,
        transition: AlertTransition,
    ) -> Result<Vec<AlertRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let mut updated = Vec::new();
                        for id in ids {
                            if let AlertTransitionOutcome::Updated(alert) =
                                transition_alert(connection, &tenant_id, &id, transition)?
                            {
                                updated.push(*alert);
                            }
                        }
                        Ok(updated)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn summary(
        &self,
        tenant: &TenantId,
    ) -> Result<Vec<(String, String, i64)>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                alert_repo::count_by_status_and_severity(connection, &tenant_id)
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn persist_cooldowns(
        &self,
        cooldowns: Vec<CooldownRecord>,
    ) -> Result<(), PersistenceError> {
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        for cooldown in cooldowns {
                            rule_repo::upsert_cooldown(
                                connection,
                                &RuleCooldown {
                                    tenant_id: cooldown.tenant_id,
                                    rule_id: cooldown.rule_id,
                                    device_id: cooldown.device_id,
                                    last_fired_at: cooldown.last_fired_at,
                                },
                            )?;
                        }
                        Ok(())
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete_resolved_before(
        &self,
        tenant: &TenantId,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                alert_repo::delete_resolved_older_than(connection, &tenant_id, cutoff)
                    .map_err(map_diesel_error)
            })
            .await
    }
}
