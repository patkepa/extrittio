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
    AlertListFilter, AlertRecord, AlertTransition, AlertTransitionOutcome, NewRuleAlertRecord,
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

#[cfg(test)]
mod fresh_database_tests {
    use super::*;
    use diesel::connection::SimpleConnection;

    #[test]
    #[ignore = "requires EXTRITTIO_TEST_EMPTY_POSTGRES_URL for a disposable empty database"]
    fn cooldowns_and_reactivation_use_the_fresh_schema() {
        let url = std::env::var("EXTRITTIO_TEST_EMPTY_POSTGRES_URL").unwrap();
        let mut connection = PgConnection::establish(&url).unwrap();
        connection.test_transaction::<_, diesel::result::Error, _>(|connection| {
            assert_eq!(crate::run_pending_migrations(connection).unwrap().len(), 1);
            connection.batch_execute(
                "INSERT INTO devices(id,name) VALUES('device','Device');
                 INSERT INTO rules(id,name,trigger_type,target_type) VALUES('rule','Rule','telemetry','global');
                 INSERT INTO alerts(id,rule_id,device_id,severity,status,message)
                 VALUES('alert','rule','device','warning','resolved','Test');",
            )?;
            let newest = chrono::DateTime::from_timestamp(20, 0).unwrap().naive_utc();
            let mut cooldown = RuleCooldown {
                tenant_id: "default".into(), rule_id: "rule".into(),
                device_id: "device".into(), last_fired_at: newest,
            };
            // Same device-lock protocol as ingestion and manual transitions.
            connection.batch_execute("SELECT id FROM devices WHERE id='device' FOR UPDATE")?;
            alert_repo::upsert_cooldown(connection, &cooldown)?;
            cooldown.last_fired_at = chrono::DateTime::from_timestamp(10, 0).unwrap().naive_utc();
            alert_repo::upsert_cooldown(connection, &cooldown)?;
            use crate::schema::rule_cooldowns;
            let stored = rule_cooldowns::table.select(rule_cooldowns::last_fired_at)
                .first::<NaiveDateTime>(connection)?;
            assert_eq!(stored, newest);
            assert!(matches!(transition_alert(connection, "default", "alert", AlertTransition::Reactivate)?, AlertTransitionOutcome::Updated(_)));
            assert_eq!(rule_cooldowns::table.count().get_result::<i64>(connection)?, 0);
            let rollback = connection.transaction::<(), diesel::result::Error, _>(|connection| {
                alert_repo::upsert_cooldown(connection, &cooldown)?;
                Err(diesel::result::Error::RollbackTransaction)
            });
            assert!(matches!(rollback, Err(diesel::result::Error::RollbackTransaction)));
            assert_eq!(rule_cooldowns::table.count().get_result::<i64>(connection)?, 0);
            Ok(())
        });
    }
}

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
    if transition.requires_active_slot() {
        if let Some(rule_id) = &alert.rule_id {
            if let Some(existing_id) = alerts::table
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
        }
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
