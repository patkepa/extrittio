use crate::{TursoConnectionHandles, row};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use extrittio_backend_core::certificates::NewDeviceCertificateRecord;
use extrittio_backend_core::devices::*;
use extrittio_backend_core::fleets::FleetRecord;
use extrittio_backend_core::{PersistenceError, TenantId};
use turso::{Connection, Row, params};

#[cfg(test)]
mod blueprint_device_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn device_crud_uses_assigned_blueprints_on_fresh_baseline() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("devices.db"),
            std::time::Duration::from_secs(1),
        )
        .await
        .unwrap();
        database.migrate().await.unwrap();
        let connection = database.shared_handles().connect().unwrap();
        connection.execute_batch(r##"
            PRAGMA foreign_keys=ON;
            INSERT INTO device_blueprints VALUES ('blueprint','default','arbitrary','Arbitrary',NULL,0,0);
            INSERT INTO device_blueprint_revisions VALUES ('revision','default','blueprint',1,
                '{"metadata":{"icon":"cube","color":"#123456"}}',
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','{}',0);
        "##).await.unwrap();
        let repository = TursoDeviceRepository::from_handles(database.shared_handles());
        let tenant = TenantId::new("default").unwrap();
        let record = CreateDeviceRecord {
            id: "device".into(),
            name: "Example".into(),
            fleet_id: None,
            firmware: "1".into(),
            contract: NewDeviceContractRecord {
                id: "contract".into(),
                blueprint_revision_id: "revision".into(),
                document: json!({"deviceId":"device"}),
                initial_configuration: Some(json!({"interval":10})),
                contract_hash: "b".repeat(64),
                created_at: Utc::now(),
            },
        };
        let created = repository
            .create(&tenant, record.clone(), None)
            .await
            .unwrap();
        assert_eq!(created.blueprint.id, "blueprint");
        assert_eq!(created.blueprint.revision_id, "revision");
        assert_eq!(created.blueprint.key, "arbitrary");
        assert_eq!(created.blueprint.icon.as_deref(), Some("cube"));
        assert_eq!(created.blueprint.color.as_deref(), Some("#123456"));
        assert_eq!(
            repository.get(&tenant, "device").await.unwrap(),
            Some(created)
        );
        let listed = repository
            .list(
                &tenant,
                DeviceListQuery {
                    status: None,
                    search: Some("arbitrary".into()),
                    fleet_id: None,
                    limit: 10,
                    offset: 0,
                },
            )
            .await
            .unwrap();
        assert_eq!(listed.total, 1);
        assert_eq!(listed.records[0].device.id, "device");
        let filter = DeviceFilter {
            status: None,
            search: Some("arbitrary".into()),
            fleet_id: None,
        };
        assert_eq!(
            repository
                .resolve_ids(&tenant, filter.clone())
                .await
                .unwrap(),
            vec!["device"]
        );
        let foreign = TenantId::new("other").unwrap();
        assert!(repository.get(&foreign, "device").await.unwrap().is_none());
        assert!(
            repository
                .resolve_ids(&foreign, filter)
                .await
                .unwrap()
                .is_empty()
        );
        let updated = repository
            .update(
                &tenant,
                "device",
                UpdateDeviceRecord {
                    name: Some("Renamed".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.device.name, "Renamed");
        assert_eq!(updated.blueprint.revision_id, "revision");
        assert!(
            repository
                .assigned_contract(&tenant, "device")
                .await
                .unwrap()
                .is_some()
        );
        let mut rejected = record;
        rejected.id = "rejected".into();
        rejected.contract.id = "rejected-contract".into();
        rejected.contract.blueprint_revision_id = "missing-revision".into();
        assert!(repository.create(&tenant, rejected, None).await.is_err());
        let mut rows = connection
            .query("SELECT count(*) FROM devices WHERE id='rejected'", ())
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            0
        );
        drop(rows);
        assert!(repository.delete(&tenant, "device").await.unwrap());
        assert!(repository.get(&tenant, "device").await.unwrap().is_none());
    }
}

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
const DEVICE_JOINS: &str = "FROM devices d
    JOIN device_contract_assignments a ON a.tenant_id=d.tenant_id AND a.device_id=d.id
    JOIN device_contracts c ON c.tenant_id=a.tenant_id AND c.device_id=a.device_id AND c.id=a.desired_contract_id
    JOIN device_blueprint_revisions r ON r.tenant_id=c.tenant_id AND r.id=c.blueprint_revision_id
    JOIN device_blueprints b ON b.tenant_id=r.tenant_id AND b.id=r.blueprint_id
    LEFT JOIN fleets f ON f.tenant_id=d.tenant_id AND f.id=d.fleet_id";

fn details_sql() -> String {
    format!(
        "SELECT d.id,d.name,d.fleet_id,d.status,d.firmware,d.last_seen,d.uptime_seconds,
        d.declared_connections,b.id,r.id,b.blueprint_key,b.name,
        json_extract(r.document,'$.metadata.icon'),json_extract(r.document,'$.metadata.color'),
        f.id,f.name {DEVICE_JOINS}"
    )
}

fn decode_details(record: &Row) -> Result<DeviceDetails, PersistenceError> {
    let connections: String = record.get(7).map_err(row::legacy_error)?;
    Ok(DeviceDetails {
        device: DeviceRecord {
            id: record.get(0).map_err(row::legacy_error)?,
            name: record.get(1).map_err(row::legacy_error)?,
            fleet_id: record
                .get::<Option<i64>>(2)
                .map_err(row::legacy_error)?
                .map(|id| row::i32(id, "devices.fleet_id"))
                .transpose()?,
            status: record.get(3).map_err(row::legacy_error)?,
            firmware: record.get(4).map_err(row::legacy_error)?,
            last_seen: record
                .get::<Option<i64>>(5)
                .map_err(row::legacy_error)?
                .map(row::datetime)
                .transpose()?,
            uptime_seconds: row::i32(
                record.get(6).map_err(row::legacy_error)?,
                "devices.uptime_seconds",
            )?,
            declared_connections: serde_json::from_str(&connections)
                .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        },
        blueprint: DeviceBlueprintIdentity {
            id: record.get(8).map_err(row::legacy_error)?,
            revision_id: record.get(9).map_err(row::legacy_error)?,
            key: record.get(10).map_err(row::legacy_error)?,
            name: record.get(11).map_err(row::legacy_error)?,
            icon: record.get(12).map_err(row::legacy_error)?,
            color: record.get(13).map_err(row::legacy_error)?,
        },
        fleet: match (
            record.get::<Option<i64>>(14).map_err(row::legacy_error)?,
            record
                .get::<Option<String>>(15)
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
    let sql = format!("{} WHERE d.tenant_id = ?1 AND d.id = ?2", details_sql());
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
            "{} WHERE d.tenant_id = ?1 AND (?2 IS NULL OR d.status = ?2) AND (?3 IS NULL OR lower(d.name) LIKE ?3 OR lower(b.name) LIKE ?3) AND (?4 IS NULL OR d.fleet_id = ?4)",
            details_sql()
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
            "INSERT INTO devices (id, tenant_id, name, fleet_id, status, firmware, uptime_seconds, declared_connections, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'offline', ?5, 0, '[]', ?6, ?6)",
            params![record.id.clone(), tenant.as_str(), record.name, record.fleet_id.map(i64::from), record.firmware, now],
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
        self.bulk_delete(tenant, vec![device_id.to_owned()])
            .await
            .map(|count| count == 1)
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
            &format!("SELECT d.id {DEVICE_JOINS}
             WHERE d.tenant_id = ?1 AND (?2 IS NULL OR d.status = ?2) AND (?3 IS NULL OR lower(d.name) LIKE ?3 OR lower(b.name) LIKE ?3) AND (?4 IS NULL OR d.fleet_id = ?4) ORDER BY d.name, d.id"),
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
            // Remove contract dependents before the device cascades its contracts.
            // Keep all deletes atomic, including any later FK rejection.
            transaction
                .execute(
                    "DELETE FROM device_contract_assignments WHERE tenant_id=?1 AND device_id=?2",
                    params![tenant.as_str(), id.clone()],
                )
                .await
                .map_err(row::legacy_error)?;
            transaction
                .execute(
                    "DELETE FROM device_events WHERE tenant_id=?1 AND device_id=?2",
                    params![tenant.as_str(), id.clone()],
                )
                .await
                .map_err(row::legacy_error)?;
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
