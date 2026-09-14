use super::PostgresAdapter;
use super::executor::map_diesel_error;
use super::outbox::enqueue_pending_actions;
use crate::db::models::{Device, NewDeviceLog};
use crate::db::schema::{device_logs, devices};
use crate::domains::devices::repository::DeviceIngressRepository;
use crate::domains::devices::types::*;
use crate::error::AppError;
use crate::persistence::PersistenceError;
use crate::tenancy::DeviceIdentity;
use async_trait::async_trait;
use diesel::sql_types::Text;
use diesel::{Connection, prelude::*};
#[derive(QueryableByName)]
struct BlueprintIdRow {
    #[diesel(sql_type = Text)]
    blueprint_id: String,
}

fn blueprint_id_for_device(
    connection: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
) -> QueryResult<Option<String>> {
    diesel::sql_query(
        "SELECT revision.blueprint_id
         FROM device_contract_assignments assignment
         JOIN device_contracts contract
           ON contract.tenant_id = assignment.tenant_id
          AND contract.id = assignment.desired_contract_id
         JOIN device_blueprint_revisions revision
           ON revision.tenant_id = contract.tenant_id
          AND revision.id = contract.blueprint_revision_id
         WHERE assignment.tenant_id = $1 AND assignment.device_id = $2",
    )
    .bind::<Text, _>(tenant_id)
    .bind::<Text, _>(device_id)
    .get_result::<BlueprintIdRow>(connection)
    .optional()
    .map(|row| row.map(|row| row.blueprint_id))
}

fn to_ingress_context(
    device: Device,
    blueprint_id: Option<String>,
) -> Result<DeviceIngressContext, PersistenceError> {
    let identity = DeviceIdentity::new(device.tenant_id, device.id).map_err(|error| {
        PersistenceError::CorruptData(format!("invalid persisted device identity: {error}"))
    })?;
    Ok(DeviceIngressContext {
        identity,
        device_type_id: device.device_type_id,
        fleet_id: device.fleet_id,
        blueprint_id,
        status: device.status,
    })
}

fn map_app_error(error: AppError) -> PersistenceError {
    match error {
        AppError::Database(error) => map_diesel_error(error),
        AppError::Persistence(error) => error,
        other => PersistenceError::Internal(other.to_string()),
    }
}

#[async_trait]
impl DeviceIngressRepository for PostgresAdapter {
    async fn resolve_identity(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceIdentity>, PersistenceError> {
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                let row = devices::table
                    .filter(devices::id.eq(device_id))
                    .select((devices::tenant_id, devices::id))
                    .first::<(String, String)>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                row.map(|(tenant_id, device_id)| {
                    DeviceIdentity::new(tenant_id, device_id).map_err(|error| {
                        PersistenceError::CorruptData(format!(
                            "invalid persisted device identity: {error}"
                        ))
                    })
                })
                .transpose()
            })
            .await
    }

    async fn ingress_context(
        &self,
        identity: &DeviceIdentity,
    ) -> Result<Option<DeviceIngressContext>, PersistenceError> {
        let tenant_id = identity.tenant_id_str().to_owned();
        let device_id = identity.device_id().to_owned();
        self.executor
            .run(move |connection| {
                let device = devices::table
                    .filter(devices::tenant_id.eq(tenant_id))
                    .filter(devices::id.eq(device_id))
                    .select(Device::as_select())
                    .first::<Device>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                let Some(device) = device else {
                    return Ok(None);
                };
                let blueprint_id =
                    blueprint_id_for_device(connection, &device.tenant_id, &device.id)
                        .map_err(map_diesel_error)?;
                to_ingress_context(device, blueprint_id).map(Some)
            })
            .await
    }

    async fn apply_heartbeat(
        &self,
        identity: &DeviceIdentity,
        write: HeartbeatWrite,
    ) -> Result<DeviceWriteOutcome, PersistenceError> {
        let tenant_id = identity.tenant_id_str().to_owned();
        let device_id = identity.device_id().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, AppError, _>(|connection| {
                        let current_status = devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::id.eq(&device_id))
                            .select(devices::status)
                            .for_update()
                            .first::<String>(connection)
                            .optional()?;
                        if current_status.as_deref() != Some(write.expected_status.as_str()) {
                            return Ok(DeviceWriteOutcome {
                                applied: false,
                                actions_enqueued: 0,
                            });
                        }
                        diesel::update(
                            devices::table
                                .filter(devices::tenant_id.eq(&tenant_id))
                                .filter(devices::id.eq(&device_id)),
                        )
                        .set((
                            devices::status.eq(&write.status),
                            devices::firmware.eq(write.firmware),
                            devices::uptime_seconds.eq(write.uptime_seconds),
                            devices::last_seen.eq(write.observed_at),
                            devices::updated_at.eq(write.observed_at),
                        ))
                        .execute(connection)?;
                        if write.expected_status != write.status {
                            diesel::insert_into(device_logs::table)
                                .values(NewDeviceLog {
                                    tenant_id: tenant_id.clone(),
                                    device_id: device_id.clone(),
                                    level: "INFO".to_string(),
                                    message: format!(
                                        "Device status changed from {} to {}",
                                        write.expected_status, write.status
                                    ),
                                })
                                .execute(connection)?;
                        }
                        let actions = crate::database::postgres_ingress_rules(
                            connection,
                            &tenant_id,
                            &device_id,
                            write.rule_evaluation.as_ref(),
                        )?;
                        let actions_enqueued = enqueue_pending_actions(connection, &actions)?;
                        Ok(DeviceWriteOutcome {
                            applied: true,
                            actions_enqueued,
                        })
                    })
                    .map_err(map_app_error)
            })
            .await
    }

    async fn offline_candidates(
        &self,
        cutoff: chrono::NaiveDateTime,
    ) -> Result<Vec<DeviceIngressContext>, PersistenceError> {
        self.executor
            .run(move |connection| {
                devices::table
                    .filter(devices::status.ne("offline"))
                    .filter(devices::last_seen.lt(cutoff))
                    .order((devices::tenant_id.asc(), devices::id.asc()))
                    .select(Device::as_select())
                    .load::<Device>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(|device| {
                        let blueprint_id =
                            blueprint_id_for_device(connection, &device.tenant_id, &device.id)
                                .map_err(map_diesel_error)?;
                        to_ingress_context(device, blueprint_id)
                    })
                    .collect()
            })
            .await
    }

    async fn apply_offline_transitions(
        &self,
        cutoff: chrono::NaiveDateTime,
        transitions: Vec<OfflineTransition>,
    ) -> Result<OfflineWriteOutcome, PersistenceError> {
        let mut transitions = transitions;
        transitions.sort_by(|a, b| {
            (
                a.context.identity.tenant_id_str(),
                a.context.identity.device_id(),
            )
                .cmp(&(
                    b.context.identity.tenant_id_str(),
                    b.context.identity.device_id(),
                ))
        });
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, AppError, _>(|connection| {
                        let mut devices_updated = 0;
                        let mut actions_enqueued = 0;
                        for transition in transitions {
                            let tenant_id = transition.context.identity.tenant_id_str();
                            let device_id = transition.context.identity.device_id();
                            let current = devices::table
                                .filter(devices::tenant_id.eq(tenant_id))
                                .filter(devices::id.eq(device_id))
                                .select((devices::status, devices::last_seen))
                                .for_update()
                                .first::<(String, Option<chrono::NaiveDateTime>)>(connection)
                                .optional()?;
                            let eligible = current.is_some_and(|(status, last_seen)| {
                                status == transition.context.status
                                    && status != "offline"
                                    && last_seen.is_some_and(|seen| seen < cutoff)
                            });
                            if !eligible {
                                continue;
                            }
                            diesel::update(
                                devices::table
                                    .filter(devices::tenant_id.eq(tenant_id))
                                    .filter(devices::id.eq(device_id)),
                            )
                            .set(devices::status.eq("offline"))
                            .execute(connection)?;
                            diesel::insert_into(device_logs::table)
                                .values(NewDeviceLog {
                                    tenant_id: tenant_id.to_string(),
                                    device_id: device_id.to_string(),
                                    level: "WARN".to_string(),
                                    message: "Device went offline (no heartbeat)".to_string(),
                                })
                                .execute(connection)?;
                            devices_updated += 1;
                            let actions = crate::database::postgres_ingress_rules(
                                connection,
                                tenant_id,
                                device_id,
                                transition.rule_evaluation.as_ref(),
                            )?;
                            actions_enqueued += enqueue_pending_actions(connection, &actions)?;
                        }
                        Ok(OfflineWriteOutcome {
                            devices_updated,
                            actions_enqueued,
                        })
                    })
                    .map_err(map_app_error)
            })
            .await
    }
}
