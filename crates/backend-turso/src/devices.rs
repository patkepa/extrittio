use crate::{TursoConnectionHandles, row};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use extrittio_backend_core::certificates::NewDeviceCertificateRecord;
use extrittio_backend_core::device_types::DeviceTypeRecord;
use extrittio_backend_core::devices::*;
use extrittio_backend_core::fleets::FleetRecord;
use extrittio_backend_core::{PersistenceError, TenantId};
use turso::{Connection, Row, params};
#[derive(Clone)]
pub struct TursoDeviceRepository {
    handles: TursoConnectionHandles,
}
impl TursoDeviceRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}
const DETAILS_SQL: &str = "SELECT d.id, d.name, d.device_type_id, d.fleet_id, d.status, d.firmware,
            d.last_seen, d.uptime_seconds, d.latest_latitude, d.latest_longitude,
            d.declared_connections, t.id, t.name, t.icon, t.color_hex, f.id, f.name
     FROM devices d JOIN device_types t
       ON t.tenant_id = d.tenant_id AND t.id = d.device_type_id
     LEFT JOIN fleets f ON f.tenant_id = d.tenant_id AND f.id = d.fleet_id";

fn decode_details(record: &Row) -> Result<DeviceDetails, PersistenceError> {
    let connections: String = record.get(10).map_err(row::legacy_error)?;
    Ok(DeviceDetails {
        device: DeviceRecord {
            id: record.get(0).map_err(row::legacy_error)?,
            name: record.get(1).map_err(row::legacy_error)?,
            device_type_id: row::i32(
                record.get(2).map_err(row::legacy_error)?,
                "devices.device_type_id",
            )?,
            fleet_id: record
                .get::<Option<i64>>(3)
                .map_err(row::legacy_error)?
                .map(|id| row::i32(id, "devices.fleet_id"))
                .transpose()?,
            status: record.get(4).map_err(row::legacy_error)?,
            firmware: record.get(5).map_err(row::legacy_error)?,
            last_seen: record
                .get::<Option<i64>>(6)
                .map_err(row::legacy_error)?
                .map(row::datetime)
                .transpose()?,
            uptime_seconds: row::i32(
                record.get(7).map_err(row::legacy_error)?,
                "devices.uptime_seconds",
            )?,
            latest_latitude: record.get(8).map_err(row::legacy_error)?,
            latest_longitude: record.get(9).map_err(row::legacy_error)?,
            declared_connections: serde_json::from_str(&connections)
                .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        },
        device_type: DeviceTypeRecord {
            id: row::i32(
                record.get(11).map_err(row::legacy_error)?,
                "device_types.id",
            )?,
            name: record.get(12).map_err(row::legacy_error)?,
            icon: record.get(13).map_err(row::legacy_error)?,
            color_hex: record.get(14).map_err(row::legacy_error)?,
        },
        fleet: match (
            record.get::<Option<i64>>(15).map_err(row::legacy_error)?,
            record
                .get::<Option<String>>(16)
                .map_err(row::legacy_error)?,
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
        .map_err(row::legacy_error)?;
    rows.next()
        .await
        .map_err(row::legacy_error)?
        .map(|record| decode_details(&record))
        .transpose()
}

#[async_trait]
impl DeviceRepository for TursoDeviceRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        query: DeviceListQuery,
    ) -> Result<DeviceList, PersistenceError> {
        let connection = self.connect()?;
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
            .map_err(row::legacy_error)?;
        let total = count_rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::legacy_error)?;
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
            .map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            records.push(decode_details(&record)?);
        }
        Ok(DeviceList { records, total })
    }

    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceDetails>, PersistenceError> {
        details_from(&self.connect()?, tenant, device_id).await
    }

    async fn assigned_contract(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceContractRecord>, PersistenceError> {
        let connection = self.connect()?;
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
            .map_err(row::legacy_error)?;
        let Some(record) = rows.next().await.map_err(row::legacy_error)? else {
            return Ok(None);
        };
        let document: String = record.get(3).map_err(row::legacy_error)?;
        Ok(Some(DeviceContractRecord {
            id: record.get(0).map_err(row::legacy_error)?,
            device_id: record.get(1).map_err(row::legacy_error)?,
            blueprint_revision_id: record.get(2).map_err(row::legacy_error)?,
            document: serde_json::from_str(&document)
                .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
            contract_hash: record.get(4).map_err(row::legacy_error)?,
            assignment_status: record.get(5).map_err(row::legacy_error)?,
            acknowledged_at: record
                .get::<Option<i64>>(6)
                .map_err(row::legacy_error)?
                .map(row::datetime)
                .transpose()?,
            error: record.get(7).map_err(row::legacy_error)?,
            created_at: row::datetime(record.get(8).map_err(row::legacy_error)?)?,
        }))
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceRecord,
        certificate: Option<NewDeviceCertificateRecord>,
    ) -> Result<DeviceDetails, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let now = Utc::now().timestamp_micros();
        let contract = record.contract;
        let initial_configuration = contract.initial_configuration;
        transaction.execute(
            "INSERT INTO devices (id, tenant_id, name, device_type_id, fleet_id, status, firmware, uptime_seconds, declared_connections, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'offline', ?6, 0, '[]', ?7, ?7)",
            params![record.id.clone(), tenant.as_str(), record.name, i64::from(record.device_type_id), record.fleet_id.map(i64::from), record.firmware, now],
        ).await.map_err(row::legacy_error)?;
        transaction.execute("INSERT INTO device_shadows (tenant_id, device_id, desired, reported, delta, version, updated_at) VALUES (?1, ?2, '{}', '{}', '{}', 1, ?3)", params![tenant.as_str(), record.id.clone(), now]).await.map_err(row::legacy_error)?;
        if let Some(certificate) = certificate {
            transaction.execute(
                "INSERT INTO device_certificates (tenant_id, device_id, private_key_pem, certificate_pem, fingerprint, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![tenant.as_str(), record.id.clone(), certificate.private_key_pem, certificate.certificate_pem, certificate.fingerprint, certificate.expires_at.timestamp_micros(), now],
            ).await.map_err(row::legacy_error)?;
        }
        {
            let document = serde_json::to_string(&contract.document)
                .map_err(|error| PersistenceError::Internal(error.to_string()))?;
            let created_at = contract.created_at.timestamp_micros();
            transaction.execute(
                "INSERT INTO device_contracts
                    (id, tenant_id, device_id, blueprint_revision_id, document, contract_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![contract.id.clone(), tenant.as_str(), record.id.clone(), contract.blueprint_revision_id, document, contract.contract_hash, created_at],
            ).await.map_err(row::legacy_error)?;
            transaction
                .execute(
                    "INSERT INTO device_contract_assignments
                    (tenant_id, device_id, desired_contract_id, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'pending', ?4, ?4)",
                    params![tenant.as_str(), record.id.clone(), contract.id, created_at],
                )
                .await
                .map_err(row::legacy_error)?;
        }
        if let Some(configuration) = initial_configuration {
            let configuration = serde_json::to_string(&configuration)
                .map_err(|error| PersistenceError::Internal(error.to_string()))?;
            transaction.execute("INSERT INTO device_configs (tenant_id, device_id, config, updated_at) VALUES (?1, ?2, ?3, ?4)", params![tenant.as_str(), record.id.clone(), configuration, now]).await.map_err(row::legacy_error)?;
        }
        let result = details_from(&transaction, tenant, &record.id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(result)
    }

    async fn update(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: UpdateDeviceRecord,
    ) -> Result<Option<DeviceDetails>, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let exists = transaction.execute("UPDATE devices SET updated_at = COALESCE(?3, updated_at) WHERE tenant_id = ?1 AND id = ?2", params![tenant.as_str(), device_id, record.updated_at.map(|value| value.timestamp_micros())]).await.map_err(row::legacy_error)?;
        if exists == 0 {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(None);
        }
        if let Some(name) = record.name {
            transaction
                .execute(
                    "UPDATE devices SET name = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), device_id, name],
                )
                .await
                .map_err(row::legacy_error)?;
        }
        if let Some(device_type_id) = record.device_type_id {
            transaction
                .execute(
                    "UPDATE devices SET device_type_id = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), device_id, i64::from(device_type_id)],
                )
                .await
                .map_err(row::legacy_error)?;
        }
        if let Some(fleet_id) = record.fleet_id {
            transaction
                .execute(
                    "UPDATE devices SET fleet_id = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), device_id, fleet_id.map(i64::from)],
                )
                .await
                .map_err(row::legacy_error)?;
        }
        if let Some(firmware) = record.firmware {
            transaction
                .execute(
                    "UPDATE devices SET firmware = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), device_id, firmware],
                )
                .await
                .map_err(row::legacy_error)?;
        }
        let result = details_from(&transaction, tenant, device_id).await?;
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(result)
    }

    async fn delete(&self, tenant: &TenantId, device_id: &str) -> Result<bool, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer
            .execute(
                "DELETE FROM devices WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), device_id],
            )
            .await
            .map(|count| count == 1)
            .map_err(row::legacy_error)
    }

    async fn resolve_ids(
        &self,
        tenant: &TenantId,
        filter: DeviceFilter,
    ) -> Result<Vec<String>, PersistenceError> {
        let search = filter
            .search
            .map(|value| format!("%{}%", value.to_ascii_lowercase()));
        let connection = self.connect()?;
        let mut rows = connection.query(
            "SELECT d.id FROM devices d JOIN device_types t ON t.tenant_id = d.tenant_id AND t.id = d.device_type_id
             WHERE d.tenant_id = ?1 AND (?2 IS NULL OR d.status = ?2) AND (?3 IS NULL OR lower(d.name) LIKE ?3 OR lower(t.name) LIKE ?3) AND (?4 IS NULL OR d.fleet_id = ?4) ORDER BY d.name, d.id",
            params![tenant.as_str(), filter.status, search, filter.fleet_id.map(i64::from)],
        ).await.map_err(row::legacy_error)?;
        let mut ids = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            ids.push(record.get(0).map_err(row::legacy_error)?);
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
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let mut count = 0;
        for id in device_ids
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
        {
            count += transaction.execute("UPDATE devices SET fleet_id = ?3, updated_at = ?4 WHERE tenant_id = ?1 AND id = ?2", params![tenant.as_str(), id, fleet_id.map(i64::from), updated_at.timestamp_micros()]).await.map_err(row::legacy_error)? as usize;
        }
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(count)
    }

    async fn bulk_delete(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
    ) -> Result<usize, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let mut count = 0;
        for id in device_ids
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
        {
            count += transaction
                .execute(
                    "DELETE FROM devices WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), id],
                )
                .await
                .map_err(row::legacy_error)? as usize;
        }
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(count)
    }
}
