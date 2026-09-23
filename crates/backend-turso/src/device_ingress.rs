use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoDeviceIngressRepository {
    handles: TursoConnectionHandles,
}
impl TursoDeviceIngressRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}
use async_trait::async_trait;
use chrono::Utc;
use extrittio_backend_core::DeviceIdentity;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::device_ingress::DeviceIngressRepository;
use extrittio_backend_core::device_ingress::*;
use extrittio_backend_core::rule_engine::types::PendingAction;
use turso::{Connection, Row, params};
const INGRESS_SELECT: &str = "SELECT d.tenant_id, d.id, d.fleet_id, d.status,
       (SELECT revision.blueprint_id
          FROM device_contract_assignments assignment
          JOIN device_contracts contract
            ON contract.tenant_id = assignment.tenant_id
           AND contract.id = assignment.desired_contract_id
          JOIN device_blueprint_revisions revision
            ON revision.tenant_id = contract.tenant_id
           AND revision.id = contract.blueprint_revision_id
         WHERE assignment.tenant_id = d.tenant_id AND assignment.device_id = d.id)
       AS blueprint_id
  FROM devices d";

fn ingress(record: &Row) -> Result<DeviceIngressContext, PersistenceError> {
    let tenant_id: String = record.get(0).map_err(row::legacy_error)?;
    let device_id: String = record.get(1).map_err(row::legacy_error)?;
    Ok(DeviceIngressContext {
        identity: DeviceIdentity::new(tenant_id, device_id)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        fleet_id: record
            .get::<Option<i64>>(2)
            .map_err(row::legacy_error)?
            .map(|id| row::i32(id, "devices.fleet_id"))
            .transpose()?,
        blueprint_id: record.get(4).map_err(row::legacy_error)?,
        status: record.get(3).map_err(row::legacy_error)?,
    })
}

pub(super) async fn enqueue(
    connection: &Connection,
    actions: &[PendingAction],
) -> Result<usize, PersistenceError> {
    crate::outbox::enqueue_actions_in_transaction(connection, actions).await
}

#[async_trait]
impl DeviceIngressRepository for TursoDeviceIngressRepository {
    async fn resolve_identity(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceIdentity>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT tenant_id, id FROM devices WHERE id = ?1",
                params![device_id],
            )
            .await
            .map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| {
                DeviceIdentity::new(
                    record.get::<String>(0).map_err(row::legacy_error)?,
                    record.get::<String>(1).map_err(row::legacy_error)?,
                )
                .map_err(|error| PersistenceError::CorruptData(error.to_string()))
            })
            .transpose()
    }

    async fn ingress_context(
        &self,
        identity: &DeviceIdentity,
    ) -> Result<Option<DeviceIngressContext>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                &format!("{INGRESS_SELECT} WHERE d.tenant_id = ?1 AND d.id = ?2"),
                params![identity.tenant_id_str(), identity.device_id()],
            )
            .await
            .map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| ingress(&record))
            .transpose()
    }

    async fn apply_heartbeat(
        &self,
        identity: &DeviceIdentity,
        write: HeartbeatWrite,
    ) -> Result<DeviceWriteOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let count = transaction.execute(
            "UPDATE devices SET status = ?4, firmware = ?5, uptime_seconds = ?6, last_seen = ?7, updated_at = ?7
             WHERE tenant_id = ?1 AND id = ?2 AND status = ?3",
            params![identity.tenant_id_str(), identity.device_id(), write.expected_status.clone(), write.status.clone(), write.firmware, i64::from(write.uptime_seconds), write.observed_at.and_utc().timestamp_micros()],
        ).await.map_err(row::legacy_error)?;
        if count == 0 {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(DeviceWriteOutcome {
                applied: false,
                actions_enqueued: 0,
            });
        }
        if write.expected_status != write.status {
            transaction.execute(
                "INSERT INTO device_logs (tenant_id, device_id, level, message, created_at) VALUES (?1, ?2, 'INFO', ?3, ?4)",
                params![identity.tenant_id_str(), identity.device_id(), format!("Device status changed from {} to {}", write.expected_status, write.status), write.observed_at.and_utc().timestamp_micros()],
            ).await.map_err(row::legacy_error)?;
        }
        let actions = crate::rule_runtime::evaluate_rules_in_transaction(
            &transaction,
            identity.tenant_id_str(),
            identity.device_id(),
            write.rule_evaluation.as_ref(),
        )
        .await?;
        let actions_enqueued = enqueue(&transaction, &actions).await?;
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(DeviceWriteOutcome {
            applied: true,
            actions_enqueued,
        })
    }

    async fn offline_candidates(
        &self,
        cutoff: chrono::NaiveDateTime,
    ) -> Result<Vec<DeviceIngressContext>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                &format!(
                    "{INGRESS_SELECT} WHERE d.status <> 'offline' AND d.last_seen < ?1
                     ORDER BY d.tenant_id, d.id"
                ),
                params![cutoff.and_utc().timestamp_micros()],
            )
            .await
            .map_err(row::legacy_error)?;
        let mut result = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            result.push(ingress(&record)?);
        }
        Ok(result)
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
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let mut devices_updated = 0;
        let mut actions_enqueued = 0;
        for transition in transitions {
            let identity = &transition.context.identity;
            let count = transaction.execute(
                "UPDATE devices SET status = 'offline' WHERE tenant_id = ?1 AND id = ?2 AND status = ?3 AND status <> 'offline' AND last_seen < ?4",
                params![identity.tenant_id_str(), identity.device_id(), transition.context.status, cutoff.and_utc().timestamp_micros()],
            ).await.map_err(row::legacy_error)?;
            if count > 0 {
                devices_updated += 1;
                transaction.execute(
                    "INSERT INTO device_logs (tenant_id, device_id, level, message, created_at) VALUES (?1, ?2, 'WARN', 'Device went offline (no heartbeat)', ?3)",
                    params![identity.tenant_id_str(), identity.device_id(), Utc::now().timestamp_micros()],
                ).await.map_err(row::legacy_error)?;
                let actions = crate::rule_runtime::evaluate_rules_in_transaction(
                    &transaction,
                    identity.tenant_id_str(),
                    identity.device_id(),
                    transition.rule_evaluation.as_ref(),
                )
                .await?;
                actions_enqueued += enqueue(&transaction, &actions).await?;
            }
        }
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(OfflineWriteOutcome {
            devices_updated,
            actions_enqueued,
        })
    }
}
