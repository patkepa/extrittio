use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use diesel::{Connection, OptionalExtension, PgConnection, prelude::*};

use crate::alerts_sql as alert_repo;
use crate::models::{Alert, NewAlert, RuleCooldown, UpdateAlert};
use crate::schema::alerts;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::alerts::AlertRepository;
use extrittio_backend_core::alerts::{
    AlertListFilter, AlertRecord, AlertTransition, AlertTransitionOutcome, CooldownRecord,
    NewRuleAlertRecord,
};

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresAlertRepository {
    executor: PostgresExecutor,
}
impl PostgresAlertRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
use crate::error::map_diesel_error;

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
    // Creators and manual transitions acquire the device before any alert row.
    let Some(before) = alert_repo::find_alert(connection, tenant_id, id).optional()? else {
        return Ok(AlertTransitionOutcome::NotFound);
    };
    use crate::schema::devices;
    if devices::table
        .filter(devices::tenant_id.eq(tenant_id))
        .filter(devices::id.eq(&before.device_id))
        .for_update()
        .select(devices::id)
        .first::<String>(connection)
        .optional()?
        .is_none()
    {
        return Ok(AlertTransitionOutcome::NotFound);
    }
    let Some(alert) = alerts::table
        .filter(alerts::tenant_id.eq(tenant_id))
        .filter(alerts::id.eq(id))
        .for_update()
        .select(Alert::as_select())
        .first(connection)
        .optional()?
    else {
        return Ok(AlertTransitionOutcome::NotFound);
    };
    let valid = transition.accepts(&alert.status);
    if !valid {
        return Ok(AlertTransitionOutcome::InvalidStatus(alert.status));
    }
    if transition.requires_active_slot()
        && let Some(rule_id) = &alert.rule_id
        && let Some(existing_id) = alerts::table
            .filter(alerts::tenant_id.eq(tenant_id))
            .filter(alerts::rule_id.eq(rule_id))
            .filter(alerts::device_id.eq(&alert.device_id))
            .filter(alerts::id.ne(id))
            .filter(alerts::status.eq_any(["active", "acknowledged"]))
            .order((alerts::created_at.desc(), alerts::id.desc()))
            .select(alerts::id)
            .first::<String>(connection)
            .optional()?
    {
        return Ok(AlertTransitionOutcome::ActiveConflict(existing_id));
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
    if let Some(rule_id) = &alert.rule_id {
        match transition {
            AlertTransition::Resolve => alert_repo::upsert_cooldown(
                connection,
                &RuleCooldown {
                    tenant_id: tenant_id.to_owned(),
                    rule_id: rule_id.clone(),
                    device_id: alert.device_id.clone(),
                    last_fired_at: now,
                },
            )?,
            AlertTransition::Reactivate => {
                diesel::sql_query("INSERT INTO rule_cooldown_resets(tenant_id,rule_id,device_id,reset_at) VALUES($1,$2,$3,$4) ON CONFLICT(tenant_id,rule_id,device_id) DO UPDATE SET reset_at=GREATEST(rule_cooldown_resets.reset_at,EXCLUDED.reset_at)")
                    .bind::<diesel::sql_types::Text,_>(tenant_id)
                    .bind::<diesel::sql_types::Text,_>(rule_id)
                    .bind::<diesel::sql_types::Text,_>(&alert.device_id)
                    .bind::<diesel::sql_types::Timestamptz,_>(now).execute(connection)?;
                use crate::schema::rule_cooldowns;
                diesel::delete(
                    rule_cooldowns::table
                        .filter(rule_cooldowns::tenant_id.eq(tenant_id))
                        .filter(rule_cooldowns::rule_id.eq(rule_id))
                        .filter(rule_cooldowns::device_id.eq(&alert.device_id)),
                )
                .execute(connection)?;
            }
            AlertTransition::Acknowledge => {}
        }
    }

    alert_repo::find_alert(connection, tenant_id, id)
        .map(alert_record)
        .map(Box::new)
        .map(AlertTransitionOutcome::Updated)
}

#[async_trait]
impl AlertRepository for PostgresAlertRepository {
    async fn create_or_get_active(
        &self,
        tenant: &TenantId,
        record: NewRuleAlertRecord,
    ) -> Result<Option<AlertRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        if let Some(applied) = find_delivery(connection, &tenant_id, &record.id)? {
                            return Ok(applied);
                        }
                        // A device row is a durable serialization point shared by
                        // all creators, including workers in other processes.
                        use crate::schema::devices;
                        devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::id.eq(&record.device_id))
                            .for_update()
                            .select(devices::id)
                            .first::<String>(connection)?;
                        if let Some(applied) = find_delivery(connection, &tenant_id, &record.id)? {
                            return Ok(applied);
                        }
                        if let Some(existing) =
                            alert_repo::find_alert(connection, &tenant_id, &record.id).optional()?
                        {
                            record_delivery(connection, &tenant_id, &record.id, &existing.id)?;
                            return Ok(Some(alert_record(existing)));
                        }
                        if let Some(existing) = alerts::table
                            .filter(alerts::tenant_id.eq(&tenant_id))
                            .filter(alerts::rule_id.eq(&record.rule_id))
                            .filter(alerts::device_id.eq(&record.device_id))
                            .filter(alerts::status.eq_any(["active", "acknowledged"]))
                            .order((alerts::created_at.desc(), alerts::id.desc()))
                            .select(Alert::as_select())
                            .first(connection)
                            .optional()?
                        {
                            record_delivery(connection, &tenant_id, &record.id, &existing.id)?;
                            return Ok(Some(alert_record(existing)));
                        }
                        alert_repo::insert_alert(
                            connection,
                            &NewAlert {
                                id: record.id.clone(),
                                tenant_id: tenant_id.clone(),
                                rule_id: Some(record.rule_id),
                                device_id: record.device_id,
                                severity: record.severity,
                                message: record.message,
                                triggered_value: record.triggered_value,
                            },
                        )?;
                        record_delivery(connection, &tenant_id, &record.id, &record.id)?;
                        alert_repo::find_alert(connection, &tenant_id, &record.id)
                            .map(alert_record)
                            .map(Some)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn update_triggered_value(
        &self,
        tenant: &TenantId,
        id: &str,
        value: String,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let id = id.to_string();
        self.executor
            .run(move |connection| {
                diesel::update(
                    alerts::table
                        .filter(alerts::tenant_id.eq(tenant_id))
                        .filter(alerts::id.eq(id)),
                )
                .set(alerts::triggered_value.eq(Some(value)))
                .execute(connection)
                .map(|rows| rows == 1)
                .map_err(map_diesel_error)
            })
            .await
    }

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
                        // Acquire every parent first in a common order; alert ID
                        // order alone can invert device locks across bulk requests.
                        use crate::schema::devices;
                        devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(
                                devices::id.eq_any(
                                    alerts::table
                                        .filter(alerts::tenant_id.eq(&tenant_id))
                                        .filter(alerts::id.eq_any(&ids))
                                        .select(alerts::device_id),
                                ),
                            )
                            .order(devices::id.asc())
                            .for_update()
                            .select(devices::id)
                            .load::<String>(connection)?;
                        let mut updated = Vec::new();
                        for id in ids.into_iter().collect::<std::collections::BTreeSet<_>>() {
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
                        // Join the same parent-lock protocol as ingress and alert
                        // transitions, including legacy queued cooldown writes.
                        use crate::schema::devices;
                        let parents = cooldowns
                            .iter()
                            .map(|c| (c.tenant_id.clone(), c.device_id.clone()))
                            .collect::<std::collections::BTreeSet<_>>();
                        for (tenant, device) in parents {
                            devices::table
                                .filter(devices::tenant_id.eq(tenant))
                                .filter(devices::id.eq(device))
                                .for_update()
                                .select(devices::id)
                                .first::<String>(connection)?;
                        }
                        for cooldown in cooldowns {
                            alert_repo::upsert_legacy_cooldown(
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

    async fn delete_all_resolved_before(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        self.executor
            .run(move |connection| {
                diesel::delete(
                    alerts::table
                        .filter(alerts::status.eq("resolved"))
                        .filter(alerts::resolved_at.lt(cutoff)),
                )
                .execute(connection)
                .map_err(map_diesel_error)
            })
            .await
    }
}

#[derive(diesel::QueryableByName)]
struct DeliveryReceipt {
    #[diesel(sql_type = diesel::sql_types::Text)]
    alert_id: String,
}
fn find_delivery(
    c: &mut PgConnection,
    tenant: &str,
    delivery: &str,
) -> QueryResult<Option<Option<AlertRecord>>> {
    let receipt = diesel::sql_query(
        "SELECT alert_id FROM rule_alert_deliveries WHERE tenant_id=$1 AND delivery_id=$2",
    )
    .bind::<diesel::sql_types::Text, _>(tenant)
    .bind::<diesel::sql_types::Text, _>(delivery)
    .get_result::<DeliveryReceipt>(c)
    .optional()?;
    receipt
        .map(|r| {
            alert_repo::find_alert(c, tenant, &r.alert_id)
                .optional()
                .map(|a| a.map(alert_record))
        })
        .transpose()
}
fn record_delivery(
    c: &mut PgConnection,
    tenant: &str,
    delivery: &str,
    alert: &str,
) -> QueryResult<()> {
    diesel::sql_query(
        "INSERT INTO rule_alert_deliveries(delivery_id,tenant_id,alert_id) VALUES($1,$2,$3)",
    )
    .bind::<diesel::sql_types::Text, _>(delivery)
    .bind::<diesel::sql_types::Text, _>(tenant)
    .bind::<diesel::sql_types::Text, _>(alert)
    .execute(c)?;
    Ok(())
}
