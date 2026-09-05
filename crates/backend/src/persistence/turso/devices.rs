use async_trait::async_trait;
use chrono::{DateTime, Utc};
use turso::{Connection, Row, params};

use crate::domains::device_types::types::DeviceTypeRecord;
use crate::domains::devices::repository::DeviceRepository;
use crate::domains::devices::types::{
    CreateDeviceRecord, DeviceContractRecord, DeviceDetails, DeviceFilter, DeviceIngressContext,
    DeviceList, DeviceListQuery, DeviceRecord, DeviceWriteOutcome, HeartbeatWrite,
    OfflineTransition, OfflineWriteOutcome, UpdateDeviceRecord,
};
use crate::domains::fleets::types::FleetRecord;
use crate::domains::identity::certificate_types::NewDeviceCertificateRecord;
use crate::persistence::PersistenceError;
use crate::rule_engine::actions::outbox_event_for_action;
use crate::rule_engine::types::PendingAction;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::{TursoAdapter, row};

const DETAILS_SQL: &str = "SELECT d.id, d.name, d.device_type_id, d.fleet_id, d.status, d.firmware,
            d.last_seen, d.uptime_seconds, d.latest_latitude, d.latest_longitude,
            d.declared_connections, t.id, t.name, t.icon, t.color_hex, f.id, f.name
     FROM devices d JOIN device_types t
       ON t.tenant_id = d.tenant_id AND t.id = d.device_type_id
     LEFT JOIN fleets f ON f.tenant_id = d.tenant_id AND f.id = d.fleet_id";

const INGRESS_SELECT: &str = "SELECT d.tenant_id, d.id, d.device_type_id, d.fleet_id, d.status,
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

fn decode_details(record: &Row) -> Result<DeviceDetails, PersistenceError> {
    let connections: String = record.get(10).map_err(row::error)?;
    Ok(DeviceDetails {
        device: DeviceRecord {
            id: record.get(0).map_err(row::error)?,
            name: record.get(1).map_err(row::error)?,
            device_type_id: row::i32(record.get(2).map_err(row::error)?, "devices.device_type_id")?,
            fleet_id: record
                .get::<Option<i64>>(3)
                .map_err(row::error)?
                .map(|id| row::i32(id, "devices.fleet_id"))
                .transpose()?,
            status: record.get(4).map_err(row::error)?,
            firmware: record.get(5).map_err(row::error)?,
            last_seen: record
                .get::<Option<i64>>(6)
                .map_err(row::error)?
                .map(row::datetime)
                .transpose()?,
            uptime_seconds: row::i32(record.get(7).map_err(row::error)?, "devices.uptime_seconds")?,
            latest_latitude: record.get(8).map_err(row::error)?,
            latest_longitude: record.get(9).map_err(row::error)?,
            declared_connections: serde_json::from_str(&connections)
                .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        },
        device_type: DeviceTypeRecord {
            id: row::i32(record.get(11).map_err(row::error)?, "device_types.id")?,
            name: record.get(12).map_err(row::error)?,
            icon: record.get(13).map_err(row::error)?,
            color_hex: record.get(14).map_err(row::error)?,
        },
        fleet: match (
            record.get::<Option<i64>>(15).map_err(row::error)?,
            record.get::<Option<String>>(16).map_err(row::error)?,
        ) {
            (None, None) => None,
            (Some(id), Some(name)) => Some(FleetRecord {
                id: row::i32(id, "fleets.id")?,
                name,
            }),
            _ => return Err(PersistenceError::CorruptData("partial fleet join".into())),
        },
    })
}

async fn details_from(
    connection: &Connection,
    tenant: &TenantId,
    device_id: &str,
) -> Result<Option<DeviceDetails>, PersistenceError> {
    let sql = format!("{DETAILS_SQL} WHERE d.tenant_id = ?1 AND d.id = ?2");
    let mut rows = connection
        .query(&sql, params![tenant.as_str(), device_id])
        .await
        .map_err(row::error)?;
    rows.next()
        .await
        .map_err(row::error)?
        .map(|record| decode_details(&record))
        .transpose()
}

fn ingress(record: &Row) -> Result<DeviceIngressContext, PersistenceError> {
    let tenant_id: String = record.get(0).map_err(row::error)?;
    let device_id: String = record.get(1).map_err(row::error)?;
    Ok(DeviceIngressContext {
        identity: DeviceIdentity::new(tenant_id, device_id)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        device_type_id: row::i32(record.get(2).map_err(row::error)?, "devices.device_type_id")?,
        fleet_id: record
            .get::<Option<i64>>(3)
            .map_err(row::error)?
            .map(|id| row::i32(id, "devices.fleet_id"))
            .transpose()?,
        blueprint_id: record.get(5).map_err(row::error)?,
        status: record.get(4).map_err(row::error)?,
    })
}

pub(super) async fn enqueue(
    connection: &Connection,
    actions: &[PendingAction],
) -> Result<usize, PersistenceError> {
    let now = Utc::now().timestamp_micros();
    let mut inserted = 0;
    for action in actions {
        let event = outbox_event_for_action(action)
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        let count = connection
            .execute(
                "INSERT INTO rule_action_outbox
             (id, tenant_id, event_type, aggregate_type, aggregate_id, idempotency_key,
              payload, status, attempts, max_attempts, available_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', 0, 10, ?8, ?8, ?8)
             ON CONFLICT DO NOTHING",
                params![
                    event.id,
                    event.tenant_id,
                    event.event_type,
                    event.aggregate_type,
                    event.aggregate_id,
                    event.idempotency_key,
                    serde_json::to_string(&event.payload)
                        .map_err(|error| PersistenceError::Internal(error.to_string()))?,
                    now
                ],
            )
            .await
            .map_err(row::error)?;
        inserted += count as usize;
    }
    Ok(inserted)
}

#[async_trait]
impl DeviceRepository for TursoAdapter {
    async fn resolve_identity(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceIdentity>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection
            .query(
                "SELECT tenant_id, id FROM devices WHERE id = ?1",
                params![device_id],
            )
            .await
            .map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| {
                DeviceIdentity::new(
                    record.get::<String>(0).map_err(row::error)?,
                    record.get::<String>(1).map_err(row::error)?,
                )
                .map_err(|error| PersistenceError::CorruptData(error.to_string()))
            })
            .transpose()
    }

    async fn ingress_context(
        &self,
        identity: &DeviceIdentity,
    ) -> Result<Option<DeviceIngressContext>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection
            .query(
                &format!("{INGRESS_SELECT} WHERE d.tenant_id = ?1 AND d.id = ?2"),
                params![identity.tenant_id_str(), identity.device_id()],
            )
            .await
            .map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| ingress(&record))
            .transpose()
    }

    async fn apply_heartbeat(
        &self,
        identity: &DeviceIdentity,
        write: HeartbeatWrite,
    ) -> Result<DeviceWriteOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let count = transaction.execute(
            "UPDATE devices SET status = ?4, firmware = ?5, uptime_seconds = ?6, last_seen = ?7, updated_at = ?7
             WHERE tenant_id = ?1 AND id = ?2 AND status = ?3",
            params![identity.tenant_id_str(), identity.device_id(), write.expected_status.clone(), write.status.clone(), write.firmware, i64::from(write.uptime_seconds), write.observed_at.and_utc().timestamp_micros()],
        ).await.map_err(row::error)?;
        if count == 0 {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(DeviceWriteOutcome {
                applied: false,
                actions_enqueued: 0,
            });
        }
        if write.expected_status != write.status {
            transaction.execute(
                "INSERT INTO device_logs (tenant_id, device_id, level, message, created_at) VALUES (?1, ?2, 'INFO', ?3, ?4)",
                params![identity.tenant_id_str(), identity.device_id(), format!("Device status changed from {} to {}", write.expected_status, write.status), write.observed_at.and_utc().timestamp_micros()],
            ).await.map_err(row::error)?;
        }
        let actions_enqueued = enqueue(&transaction, &write.pending_actions).await?;
        transaction.commit().await.map_err(row::error)?;
        Ok(DeviceWriteOutcome {
            applied: true,
            actions_enqueued,
        })
    }

    async fn offline_candidates(
        &self,
        cutoff: chrono::NaiveDateTime,
    ) -> Result<Vec<DeviceIngressContext>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection
            .query(
                &format!(
                    "{INGRESS_SELECT} WHERE d.status <> 'offline' AND d.last_seen < ?1
                     ORDER BY d.tenant_id, d.id"
                ),
                params![cutoff.and_utc().timestamp_micros()],
            )
            .await
            .map_err(row::error)?;
        let mut result = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            result.push(ingress(&record)?);
        }
        Ok(result)
    }

    async fn apply_offline_transitions(
        &self,
        cutoff: chrono::NaiveDateTime,
        transitions: Vec<OfflineTransition>,
    ) -> Result<OfflineWriteOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let mut devices_updated = 0;
        let mut actions_enqueued = 0;
        for transition in transitions {
            let identity = &transition.context.identity;
            let count = transaction.execute(
                "UPDATE devices SET status = 'offline' WHERE tenant_id = ?1 AND id = ?2 AND status = ?3 AND status <> 'offline' AND last_seen < ?4",
                params![identity.tenant_id_str(), identity.device_id(), transition.context.status, cutoff.and_utc().timestamp_micros()],
            ).await.map_err(row::error)?;
            if count > 0 {
                devices_updated += 1;
                transaction.execute(
                    "INSERT INTO device_logs (tenant_id, device_id, level, message, created_at) VALUES (?1, ?2, 'WARN', 'Device went offline (no heartbeat)', ?3)",
                    params![identity.tenant_id_str(), identity.device_id(), Utc::now().timestamp_micros()],
                ).await.map_err(row::error)?;
                actions_enqueued += enqueue(&transaction, &transition.pending_actions).await?;
            }
        }
        transaction.commit().await.map_err(row::error)?;
        Ok(OfflineWriteOutcome {
            devices_updated,
            actions_enqueued,
        })
    }

    async fn list(
        &self,
        tenant: &TenantId,
        query: DeviceListQuery,
    ) -> Result<DeviceList, PersistenceError> {
        let connection = self.database.connect()?;
        let search = query
            .search
            .map(|value| format!("%{}%", value.to_ascii_lowercase()));
        let base = format!(
            "{DETAILS_SQL} WHERE d.tenant_id = ?1 AND (?2 IS NULL OR d.status = ?2) AND (?3 IS NULL OR lower(d.name) LIKE ?3 OR lower(t.name) LIKE ?3) AND (?4 IS NULL OR d.fleet_id = ?4)"
        );
        let mut count_rows = connection
            .query(
                &format!("SELECT count(*) FROM ({base}) filtered"),
                params![
                    tenant.as_str(),
                    query.status.clone(),
                    search.clone(),
                    query.fleet_id.map(i64::from)
                ],
            )
            .await
            .map_err(row::error)?;
        let total = count_rows
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::error)?;
        let mut rows = connection
            .query(
                &format!("{base} ORDER BY d.name, d.id LIMIT ?5 OFFSET ?6"),
                params![
                    tenant.as_str(),
                    query.status,
                    search,
                    query.fleet_id.map(i64::from),
                    query.limit,
                    query.offset
                ],
            )
            .await
            .map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            records.push(decode_details(&record)?);
        }
        Ok(DeviceList { records, total })
    }

    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceDetails>, PersistenceError> {
        details_from(&self.database.connect()?, tenant, device_id).await
    }

    async fn assigned_contract(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceContractRecord>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection
            .query(
                "SELECT c.id, c.device_id, c.blueprint_revision_id, c.document,
                        c.contract_hash, a.status, a.acknowledged_at, a.error, c.created_at
                 FROM device_contract_assignments a
                 JOIN device_contracts c
                   ON c.tenant_id = a.tenant_id AND c.id = a.desired_contract_id
                 WHERE a.tenant_id = ?1 AND a.device_id = ?2",
                params![tenant.as_str(), device_id],
            )
            .await
            .map_err(row::error)?;
        let Some(record) = rows.next().await.map_err(row::error)? else {
            return Ok(None);
        };
        let document: String = record.get(3).map_err(row::error)?;
        Ok(Some(DeviceContractRecord {
            id: record.get(0).map_err(row::error)?,
            device_id: record.get(1).map_err(row::error)?,
            blueprint_revision_id: record.get(2).map_err(row::error)?,
            document: serde_json::from_str(&document)
                .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
            contract_hash: record.get(4).map_err(row::error)?,
            assignment_status: record.get(5).map_err(row::error)?,
            acknowledged_at: record
                .get::<Option<i64>>(6)
                .map_err(row::error)?
                .map(row::datetime)
                .transpose()?,
            error: record.get(7).map_err(row::error)?,
            created_at: row::datetime(record.get(8).map_err(row::error)?)?,
        }))
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceRecord,
        certificate: Option<NewDeviceCertificateRecord>,
    ) -> Result<DeviceDetails, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let now = Utc::now().timestamp_micros();
        let contract = record.contract;
        transaction.execute(
            "INSERT INTO devices (id, tenant_id, name, device_type_id, fleet_id, status, firmware, uptime_seconds, declared_connections, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'offline', ?6, 0, '[]', ?7, ?7)",
            params![record.id.clone(), tenant.as_str(), record.name, i64::from(record.device_type_id), record.fleet_id.map(i64::from), record.firmware, now],
        ).await.map_err(row::error)?;
        transaction.execute("INSERT INTO device_shadows (tenant_id, device_id, desired, reported, delta, version, updated_at) VALUES (?1, ?2, '{}', '{}', '{}', 1, ?3)", params![tenant.as_str(), record.id.clone(), now]).await.map_err(row::error)?;
        if let Some(certificate) = certificate {
            transaction.execute(
                "INSERT INTO device_certificates (tenant_id, device_id, private_key_pem, certificate_pem, fingerprint, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![tenant.as_str(), record.id.clone(), certificate.private_key_pem, certificate.certificate_pem, certificate.fingerprint, certificate.expires_at.timestamp_micros(), now],
            ).await.map_err(row::error)?;
        }
        if let Some(contract) = contract {
            let document = serde_json::to_string(&contract.document)
                .map_err(|error| PersistenceError::Internal(error.to_string()))?;
            let created_at = contract.created_at.timestamp_micros();
            transaction.execute(
                "INSERT INTO device_contracts
                    (id, tenant_id, device_id, blueprint_revision_id, document, contract_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![contract.id.clone(), tenant.as_str(), record.id.clone(), contract.blueprint_revision_id, document, contract.contract_hash, created_at],
            ).await.map_err(row::error)?;
            transaction
                .execute(
                    "INSERT INTO device_contract_assignments
                    (tenant_id, device_id, desired_contract_id, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'pending', ?4, ?4)",
                    params![tenant.as_str(), record.id.clone(), contract.id, created_at],
                )
                .await
                .map_err(row::error)?;
        }
        let result = details_from(&transaction, tenant, &record.id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        transaction.commit().await.map_err(row::error)?;
        Ok(result)
    }

    async fn update(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: UpdateDeviceRecord,
    ) -> Result<Option<DeviceDetails>, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let exists = transaction.execute("UPDATE devices SET updated_at = COALESCE(?3, updated_at) WHERE tenant_id = ?1 AND id = ?2", params![tenant.as_str(), device_id, record.updated_at.map(|value| value.timestamp_micros())]).await.map_err(row::error)?;
        if exists == 0 {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(None);
        }
        if let Some(name) = record.name {
            transaction
                .execute(
                    "UPDATE devices SET name = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), device_id, name],
                )
                .await
                .map_err(row::error)?;
        }
        if let Some(device_type_id) = record.device_type_id {
            transaction
                .execute(
                    "UPDATE devices SET device_type_id = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), device_id, i64::from(device_type_id)],
                )
                .await
                .map_err(row::error)?;
        }
        if let Some(fleet_id) = record.fleet_id {
            transaction
                .execute(
                    "UPDATE devices SET fleet_id = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), device_id, fleet_id.map(i64::from)],
                )
                .await
                .map_err(row::error)?;
        }
        if let Some(firmware) = record.firmware {
            transaction
                .execute(
                    "UPDATE devices SET firmware = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), device_id, firmware],
                )
                .await
                .map_err(row::error)?;
        }
        let result = details_from(&transaction, tenant, device_id).await?;
        transaction.commit().await.map_err(row::error)?;
        Ok(result)
    }

    async fn delete(&self, tenant: &TenantId, device_id: &str) -> Result<bool, PersistenceError> {
        let writer = self.database.writer().await;
        writer
            .execute(
                "DELETE FROM devices WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), device_id],
            )
            .await
            .map(|count| count == 1)
            .map_err(row::error)
    }

    async fn resolve_ids(
        &self,
        tenant: &TenantId,
        filter: DeviceFilter,
    ) -> Result<Vec<String>, PersistenceError> {
        let search = filter
            .search
            .map(|value| format!("%{}%", value.to_ascii_lowercase()));
        let connection = self.database.connect()?;
        let mut rows = connection.query(
            "SELECT d.id FROM devices d JOIN device_types t ON t.tenant_id = d.tenant_id AND t.id = d.device_type_id
             WHERE d.tenant_id = ?1 AND (?2 IS NULL OR d.status = ?2) AND (?3 IS NULL OR lower(d.name) LIKE ?3 OR lower(t.name) LIKE ?3) AND (?4 IS NULL OR d.fleet_id = ?4) ORDER BY d.name, d.id",
            params![tenant.as_str(), filter.status, search, filter.fleet_id.map(i64::from)],
        ).await.map_err(row::error)?;
        let mut ids = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            ids.push(record.get(0).map_err(row::error)?);
        }
        Ok(ids)
    }

    async fn bulk_assign_fleet(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
        fleet_id: Option<i32>,
        updated_at: DateTime<Utc>,
    ) -> Result<usize, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let mut count = 0;
        for id in device_ids {
            count += transaction.execute("UPDATE devices SET fleet_id = ?3, updated_at = ?4 WHERE tenant_id = ?1 AND id = ?2", params![tenant.as_str(), id, fleet_id.map(i64::from), updated_at.timestamp_micros()]).await.map_err(row::error)? as usize;
        }
        transaction.commit().await.map_err(row::error)?;
        Ok(count)
    }

    async fn bulk_delete(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
    ) -> Result<usize, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let mut count = 0;
        for id in device_ids {
            count += transaction
                .execute(
                    "DELETE FROM devices WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), id],
                )
                .await
                .map_err(row::error)? as usize;
        }
        transaction.commit().await.map_err(row::error)?;
        Ok(count)
    }
}
