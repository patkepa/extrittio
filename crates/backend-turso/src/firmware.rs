use async_trait::async_trait;
use turso::{Connection, Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::firmware::FirmwareRepository;
use extrittio_backend_core::firmware::{
    FirmwareBlobRecord, FirmwarePage, FirmwareRecord, GlobalOtaDeploymentPage,
    GlobalOtaDeploymentRecord, NewFirmwareBlobRecord, NewFirmwareRecord, OtaDeploymentPage,
    OtaDeploymentRecord, OtaStatusUpdate, TriggerOtaOutcome,
};
use extrittio_backend_core::{DeviceIdentity, TenantId};

use crate::{TursoConnectionHandles, row};

#[cfg(test)]
mod blueprint_ota_tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn global_deployments_use_artifact_revisions_without_device_types() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("deployments.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let connection = database.shared_handles().connect().unwrap();
        connection.execute_batch(
            "CREATE TABLE devices (tenant_id TEXT, id TEXT, name TEXT, status TEXT, firmware TEXT, fleet_id INTEGER);
             CREATE TABLE firmware_updates (tenant_id TEXT, id INTEGER, version TEXT, blueprint_revision_id TEXT);
             CREATE TABLE fleets (tenant_id TEXT, id INTEGER, name TEXT);
             CREATE TABLE ota_deployments (tenant_id TEXT, id INTEGER, device_id TEXT,
                firmware_update_id INTEGER, status TEXT, error_message TEXT,
                initiated_at INTEGER, completed_at INTEGER);
             INSERT INTO devices VALUES ('tenant', 'device', 'Sensor', 'online', '1', 7);
             INSERT INTO devices VALUES ('other', 'device', 'Wrong device', 'online', '1', 7);
             INSERT INTO firmware_updates VALUES ('tenant', 1, '2', 'revision');
             INSERT INTO firmware_updates VALUES ('other', 1, 'wrong-version', 'wrong-revision');
             INSERT INTO fleets VALUES ('tenant', 7, 'Fleet');
             INSERT INTO fleets VALUES ('other', 7, 'Wrong fleet');
             INSERT INTO ota_deployments VALUES ('tenant', 1, 'device', 1, 'success', NULL, 1000000, 2000000);
             INSERT INTO ota_deployments VALUES ('tenant', 2, 'device', 1, 'pending', NULL, 1000000, NULL);"
        ).await.unwrap();
        let repository = TursoFirmwareRepository::from_handles(database.shared_handles());
        let tenant = TenantId::new("tenant").unwrap();
        let all = repository
            .list_all_deployments(&tenant, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(all.total, 2);
        assert_eq!(all.records.len(), 2);
        assert_eq!(all.records[0].id, 2);
        let record = &all.records[1];
        assert_eq!(record.device_id, "device");
        assert_eq!(record.device_name, "Sensor");
        assert_eq!(record.device_status, "online");
        assert_eq!(record.current_firmware, "1");
        assert_eq!(record.blueprint_revision_id, "revision");
        assert_eq!(record.fleet_id, Some(7));
        assert_eq!(record.fleet_name.as_deref(), Some("Fleet"));
        assert_eq!(record.firmware_update_id, 1);
        assert_eq!(record.firmware_version, "2");
        assert_eq!(record.status, "success");
        assert!(record.error_message.is_none());
        assert_eq!(record.initiated_at.and_utc().timestamp(), 1);
        assert_eq!(record.completed_at.unwrap().and_utc().timestamp(), 2);
        for status in ["completed", "success", "in_progress"] {
            let page = repository
                .list_all_deployments(&tenant, Some(status.into()), 1, 0)
                .await
                .unwrap();
            assert_eq!(page.total, 1);
            assert_eq!(page.records.len(), 1);
        }
        let next = repository
            .list_all_deployments(&tenant, None, 1, 1)
            .await
            .unwrap();
        assert_eq!(next.records[0].id, 1);
        let other = repository
            .list_all_deployments(&TenantId::new("other").unwrap(), None, 10, 0)
            .await
            .unwrap();
        assert_eq!(other.total, 0);
        assert!(other.records.is_empty());
    }

    #[tokio::test]
    async fn firmware_reads_need_no_device_types_and_preserve_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("firmware.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let connection = database.shared_handles().connect().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE firmware_updates (
                tenant_id TEXT, id INTEGER, version TEXT, url TEXT, sha256 TEXT,
                description TEXT, created_at INTEGER, commit_sha TEXT, branch TEXT,
                ci_run_url TEXT, build_timestamp INTEGER, changelog TEXT, source TEXT,
                blueprint_revision_id TEXT, compatibility TEXT, update_strategy TEXT);
             CREATE TABLE firmware_blobs (
                tenant_id TEXT, firmware_update_id INTEGER, size INTEGER, filename TEXT);
             INSERT INTO firmware_updates VALUES (
                'tenant', 1, '1.2.3', 'https://example.test/fw', 'hash', 'description',
                1000000, 'commit', 'branch', 'ci-url', 2000000, 'changes', 'ci',
                'revision', '{}', 'ota');
             INSERT INTO firmware_blobs VALUES ('other', 1, 999, 'wrong.bin');
             INSERT INTO firmware_blobs VALUES ('tenant', 1, 42, 'right.bin');",
            )
            .await
            .unwrap();
        let mut rows = connection
            .query(
                &format!("{FIRMWARE_SELECT} WHERE f.tenant_id=?1"),
                params!["tenant"],
            )
            .await
            .unwrap();
        let record = firmware(&rows.next().await.unwrap().unwrap()).unwrap();
        assert_eq!(record.id, 1);
        assert_eq!(record.version, "1.2.3");
        assert_eq!(record.url, "https://example.test/fw");
        assert_eq!(record.sha256.as_deref(), Some("hash"));
        assert_eq!(record.description.as_deref(), Some("description"));
        assert_eq!(record.created_at.and_utc().timestamp(), 1);
        assert_eq!(record.file_size, Some(42));
        assert_eq!(record.filename.as_deref(), Some("right.bin"));
        assert_eq!(record.commit_sha.as_deref(), Some("commit"));
        assert_eq!(record.branch.as_deref(), Some("branch"));
        assert_eq!(record.ci_run_url.as_deref(), Some("ci-url"));
        assert_eq!(record.build_timestamp.unwrap().and_utc().timestamp(), 2);
        assert_eq!(record.changelog.as_deref(), Some("changes"));
        assert_eq!(record.source, "ci");
        assert_eq!(record.blueprint_revision_id, "revision");
        assert_eq!(record.compatibility, serde_json::json!({}));
        assert_eq!(record.update_strategy.as_deref(), Some("ota"));
        assert!(rows.next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn ota_rejects_missing_or_mismatched_revisions_before_any_deployment_write() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("ota.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let connection = database.shared_handles().connect().unwrap();
        // Deliberately no device-type columns or deployment/shadow tables: rejected
        // requests must not need legacy identity or attempt downstream writes.
        connection.execute_batch(
            "CREATE TABLE devices (tenant_id TEXT, id TEXT);
             CREATE TABLE firmware_updates (tenant_id TEXT, id INTEGER, version TEXT, url TEXT, sha256 TEXT, blueprint_revision_id TEXT);
             CREATE TABLE device_contract_assignments (tenant_id TEXT, device_id TEXT, desired_contract_id TEXT);
             CREATE TABLE device_contracts (tenant_id TEXT, id TEXT, blueprint_revision_id TEXT);
             INSERT INTO devices VALUES ('tenant', 'device');
             INSERT INTO firmware_updates VALUES ('tenant', 1, '1.0.0', 'https://example.test/fw.bin', NULL, 'revision-2');
             INSERT INTO device_contract_assignments VALUES ('tenant', 'device', 'contract');
             INSERT INTO device_contracts VALUES ('tenant', 'contract', 'revision-1');"
        ).await.unwrap();
        let repository = TursoFirmwareRepository::from_handles(database.shared_handles());
        let tenant = TenantId::new("tenant").unwrap();
        for revision in ["revision-2", ""] {
            connection
                .execute(
                    "UPDATE firmware_updates SET blueprint_revision_id = ?1",
                    params![revision],
                )
                .await
                .unwrap();
            let outcome = repository
                .trigger_ota(&tenant, "device", 1, "https://example.test/fw.bin")
                .await
                .unwrap();
            assert!(matches!(outcome, TriggerOtaOutcome::Incompatible));
        }
        connection
            .execute(
                "UPDATE firmware_updates SET blueprint_revision_id = 'revision-1'",
                (),
            )
            .await
            .unwrap();
        let matching = repository
            .trigger_ota(&tenant, "device", 1, "https://example.test/fw.bin")
            .await
            .unwrap();
        assert!(matches!(matching, TriggerOtaOutcome::InvalidArtifact));
        connection
            .execute("DELETE FROM device_contract_assignments", ())
            .await
            .unwrap();
        let unassigned = repository
            .trigger_ota(&tenant, "device", 1, "https://example.test/fw.bin")
            .await
            .unwrap();
        assert!(matches!(unassigned, TriggerOtaOutcome::Incompatible));
        let other = TenantId::new("other").unwrap();
        assert!(matches!(
            repository
                .trigger_ota(&other, "device", 1, "https://example.test/fw.bin")
                .await
                .unwrap(),
            TriggerOtaOutcome::DeviceNotFound
        ));
    }
}

#[derive(Clone)]
pub struct TursoFirmwareRepository {
    handles: TursoConnectionHandles,
}
impl TursoFirmwareRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}

fn firmware(r: &Row) -> Result<FirmwareRecord, PersistenceError> {
    Ok(FirmwareRecord {
        id: row::i32(r.get(0).map_err(row::legacy_error)?, "firmware.id")?,
        version: r.get(1).map_err(row::legacy_error)?,
        url: r.get(2).map_err(row::legacy_error)?,
        sha256: r.get(3).map_err(row::legacy_error)?,
        description: r.get(4).map_err(row::legacy_error)?,
        created_at: row::datetime(r.get(5).map_err(row::legacy_error)?)?.naive_utc(),
        file_size: r
            .get::<Option<i64>>(6)
            .map_err(row::legacy_error)?
            .map(|v| row::i32(v, "blob.size"))
            .transpose()?,
        filename: r.get(7).map_err(row::legacy_error)?,
        commit_sha: r.get(8).map_err(row::legacy_error)?,
        branch: r.get(9).map_err(row::legacy_error)?,
        ci_run_url: r.get(10).map_err(row::legacy_error)?,
        build_timestamp: r
            .get::<Option<i64>>(11)
            .map_err(row::legacy_error)?
            .map(row::datetime)
            .transpose()?
            .map(|v| v.naive_utc()),
        changelog: r.get(12).map_err(row::legacy_error)?,
        source: r.get(13).map_err(row::legacy_error)?,
        blueprint_revision_id: r.get(14).map_err(row::legacy_error)?,
        compatibility: serde_json::from_str::<serde_json::Value>(
            &r.get::<String>(15).map_err(row::legacy_error)?,
        )
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        update_strategy: r.get(16).map_err(row::legacy_error)?,
    })
}

const FIRMWARE_SELECT: &str = "SELECT f.id,f.version,f.url,f.sha256,f.description,f.created_at,b.size,b.filename,f.commit_sha,f.branch,f.ci_run_url,f.build_timestamp,f.changelog,f.source,f.blueprint_revision_id,f.compatibility,f.update_strategy FROM firmware_updates f LEFT JOIN firmware_blobs b ON b.tenant_id=f.tenant_id AND b.firmware_update_id=f.id";

async fn scalar(
    c: &Connection,
    sql: &str,
    p: impl turso::IntoParams,
) -> Result<i64, PersistenceError> {
    let mut rs = c.query(sql, p).await.map_err(row::legacy_error)?;
    rs.next()
        .await
        .map_err(row::legacy_error)?
        .ok_or(PersistenceError::NotFound)?
        .get(0)
        .map_err(row::legacy_error)
}
fn blob(r: &Row) -> Result<FirmwareBlobRecord, PersistenceError> {
    Ok(FirmwareBlobRecord {
        size: row::i32(r.get(0).map_err(row::legacy_error)?, "blob.size")?,
        filename: r.get(1).map_err(row::legacy_error)?,
        storage_key: r.get(2).map_err(row::legacy_error)?,
        storage_backend: r.get(3).map_err(row::legacy_error)?,
    })
}

async fn insert_firmware(
    c: &Connection,
    tenant: &str,
    r: NewFirmwareRecord,
) -> Result<i32, PersistenceError> {
    let compatibility = serde_json::to_string(&r.compatibility)
        .map_err(|error| PersistenceError::Internal(error.to_string()))?;
    c.execute("INSERT INTO firmware_updates(tenant_id,version,url,description,sha256,commit_sha,branch,ci_run_url,build_timestamp,changelog,source,created_at,blueprint_revision_id,compatibility,update_strategy)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![tenant,r.version,r.url,r.description,r.sha256,r.commit_sha,r.branch,r.ci_run_url,r.build_timestamp.map(|v|v.and_utc().timestamp_micros()),r.changelog,r.source.unwrap_or_else(||"manual".into()),chrono::Utc::now().timestamp_micros(),r.blueprint_revision_id,compatibility,r.update_strategy]).await.map_err(row::legacy_error)?;
    row::i32(
        scalar(c, "SELECT last_insert_rowid()", ()).await?,
        "firmware.id",
    )
}

async fn find_firmware(
    c: &Connection,
    tenant: &str,
    id: i32,
) -> Result<Option<FirmwareRecord>, PersistenceError> {
    let mut rs = c
        .query(
            &format!("{FIRMWARE_SELECT} WHERE f.tenant_id=?1 AND f.id=?2"),
            params![tenant, id],
        )
        .await
        .map_err(row::legacy_error)?;
    rs.next()
        .await
        .map_err(row::legacy_error)?
        .map(|r| firmware(&r))
        .transpose()
}

#[async_trait]
impl FirmwareRepository for TursoFirmwareRepository {
    async fn list(
        &self,
        t: &TenantId,
        blueprint_revision_id: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<FirmwarePage, PersistenceError> {
        let c = self.connect()?;
        let total=scalar(&c,"SELECT count(*) FROM firmware_updates WHERE tenant_id=?1 AND (?2 IS NULL OR blueprint_revision_id=?2)",params![t.as_str(),blueprint_revision_id.clone()]).await?;
        let mut rs=c.query(&format!("{FIRMWARE_SELECT} WHERE f.tenant_id=?1 AND (?2 IS NULL OR f.blueprint_revision_id=?2) ORDER BY f.created_at DESC,f.id DESC LIMIT ?3 OFFSET ?4"),params![t.as_str(),blueprint_revision_id,limit,offset]).await.map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(r) = rs.next().await.map_err(row::legacy_error)? {
            records.push(firmware(&r)?)
        }
        Ok(FirmwarePage { records, total })
    }
    async fn list_all_deployments(
        &self,
        t: &TenantId,
        status: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<GlobalOtaDeploymentPage, PersistenceError> {
        let c = self.connect()?;
        let condition = match status.as_deref() {
            Some("in_progress" | "active") => " AND o.status NOT IN ('success','failed')",
            Some("completed" | "terminal") => " AND o.status IN ('success','failed')",
            Some(v) if !v.is_empty() && v != "all" => " AND o.status=?2",
            _ => "",
        };
        let exact = if condition.contains("?2") {
            status
        } else {
            None
        };
        let count_sql =
            format!("SELECT count(*) FROM ota_deployments o WHERE o.tenant_id=?1{condition}");
        let total = if exact.is_some() {
            scalar(&c, &count_sql, params![t.as_str(), exact.clone()]).await?
        } else {
            scalar(&c, &count_sql, params![t.as_str()]).await?
        };
        let select = "SELECT o.id,d.id,d.name,d.status,d.firmware,f.blueprint_revision_id,fl.id,fl.name,o.firmware_update_id,f.version,o.status,o.error_message,o.initiated_at,o.completed_at FROM ota_deployments o JOIN firmware_updates f ON f.tenant_id=o.tenant_id AND f.id=o.firmware_update_id JOIN devices d ON d.tenant_id=o.tenant_id AND d.id=o.device_id LEFT JOIN fleets fl ON fl.tenant_id=d.tenant_id AND fl.id=d.fleet_id";
        let mut rs = if exact.is_some() {
            c.query(
                &format!("{select} WHERE o.tenant_id=?1{condition} ORDER BY o.initiated_at DESC,o.id DESC LIMIT ?3 OFFSET ?4"),
                params![t.as_str(), exact, limit, offset],
            )
            .await
        } else {
            c.query(
                &format!("{select} WHERE o.tenant_id=?1{condition} ORDER BY o.initiated_at DESC,o.id DESC LIMIT ?2 OFFSET ?3"),
                params![t.as_str(), limit, offset],
            )
            .await
        }
        .map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(r) = rs.next().await.map_err(row::legacy_error)? {
            records.push(GlobalOtaDeploymentRecord {
                id: row::i32(r.get(0).map_err(row::legacy_error)?, "deployment.id")?,
                device_id: r.get(1).map_err(row::legacy_error)?,
                device_name: r.get(2).map_err(row::legacy_error)?,
                device_status: r.get(3).map_err(row::legacy_error)?,
                current_firmware: r.get(4).map_err(row::legacy_error)?,
                blueprint_revision_id: r.get(5).map_err(row::legacy_error)?,
                fleet_id: r
                    .get::<Option<i64>>(6)
                    .map_err(row::legacy_error)?
                    .map(|v| row::i32(v, "fleet.id"))
                    .transpose()?,
                fleet_name: r.get(7).map_err(row::legacy_error)?,
                firmware_update_id: row::i32(r.get(8).map_err(row::legacy_error)?, "firmware.id")?,
                firmware_version: r.get(9).map_err(row::legacy_error)?,
                status: r.get(10).map_err(row::legacy_error)?,
                error_message: r.get(11).map_err(row::legacy_error)?,
                initiated_at: row::datetime(r.get(12).map_err(row::legacy_error)?)?.naive_utc(),
                completed_at: r
                    .get::<Option<i64>>(13)
                    .map_err(row::legacy_error)?
                    .map(row::datetime)
                    .transpose()?
                    .map(|v| v.naive_utc()),
            })
        }
        Ok(GlobalOtaDeploymentPage { records, total })
    }
    async fn create(
        &self,
        t: &TenantId,
        r: NewFirmwareRecord,
        b: Option<NewFirmwareBlobRecord>,
    ) -> Result<Option<FirmwareRecord>, PersistenceError> {
        let mut w = self.handles.lock_writer().await;
        let tx = w.transaction().await.map_err(row::legacy_error)?;
        if scalar(
            &tx,
            "SELECT count(*) FROM device_blueprint_revisions WHERE tenant_id=?1 AND id=?2",
            params![t.as_str(), r.blueprint_revision_id.clone()],
        )
        .await?
            == 0
        {
            tx.rollback().await.map_err(row::legacy_error)?;
            return Ok(None);
        }
        let id = insert_firmware(&tx, t.as_str(), r).await?;
        if let Some(b) = b {
            tx.execute("INSERT INTO firmware_blobs(firmware_update_id,tenant_id,size,filename,storage_key,storage_backend,created_at)VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,t.as_str(),b.size,b.filename,b.storage_key,b.storage_backend,chrono::Utc::now().timestamp_micros()]).await.map_err(row::legacy_error)?;
            tx.execute(
                "UPDATE firmware_updates SET url=?3 WHERE tenant_id=?1 AND id=?2",
                params![
                    t.as_str(),
                    id,
                    format!("/api/v1/firmware-updates/{id}/download")
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        }
        let out = find_firmware(&tx, t.as_str(), id).await?;
        tx.commit().await.map_err(row::legacy_error)?;
        Ok(out)
    }
    async fn next_blueprint_version(
        &self,
        t: &TenantId,
        blueprint_revision_id: &str,
    ) -> Result<String, PersistenceError> {
        let c = self.connect()?;
        let mut rs=c.query("SELECT version FROM firmware_updates WHERE tenant_id=?1 AND blueprint_revision_id=?2 ORDER BY created_at DESC,id DESC LIMIT 1",params![t.as_str(),blueprint_revision_id]).await.map_err(row::legacy_error)?;
        Ok(rs
            .next()
            .await
            .map_err(row::legacy_error)?
            .map(|r| r.get::<String>(0).map_err(row::legacy_error))
            .transpose()?
            .map_or_else(
                || "1.0.0".into(),
                |v| extrittio_backend_core::firmware::increment_firmware_version(&v),
            ))
    }
    async fn get_blob(
        &self,
        t: &TenantId,
        id: i32,
    ) -> Result<Option<FirmwareBlobRecord>, PersistenceError> {
        let c = self.connect()?;
        let mut rs=c.query("SELECT size,filename,storage_key,storage_backend FROM firmware_blobs WHERE tenant_id=?1 AND firmware_update_id=?2",params![t.as_str(),id]).await.map_err(row::legacy_error)?;
        rs.next()
            .await
            .map_err(row::legacy_error)?
            .map(|r| blob(&r))
            .transpose()
    }
    async fn delete(
        &self,
        t: &TenantId,
        id: i32,
    ) -> Result<Option<Option<FirmwareBlobRecord>>, PersistenceError> {
        let mut w = self.handles.lock_writer().await;
        let tx = w.transaction().await.map_err(row::legacy_error)?;
        let mut rs=tx.query("SELECT size,filename,storage_key,storage_backend FROM firmware_blobs WHERE tenant_id=?1 AND firmware_update_id=?2",params![t.as_str(),id]).await.map_err(row::legacy_error)?;
        let b = rs
            .next()
            .await
            .map_err(row::legacy_error)?
            .map(|r| blob(&r))
            .transpose()?;
        drop(rs);
        let n = tx
            .execute(
                "DELETE FROM firmware_updates WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id],
            )
            .await
            .map_err(row::legacy_error)?;
        tx.commit().await.map_err(row::legacy_error)?;
        Ok((n > 0).then_some(b))
    }
    async fn apply_ota_status(
        &self,
        i: &DeviceIdentity,
        u: OtaStatusUpdate,
    ) -> Result<bool, PersistenceError> {
        let mut w = self.handles.lock_writer().await;
        let tx = w.transaction().await.map_err(row::legacy_error)?;
        let mut rs=tx.query("SELECT id,status,firmware_update_id FROM ota_deployments WHERE tenant_id=?1 AND device_id=?2 AND id=?3",params![i.tenant_id_str(),i.device_id(),u.deployment_id]).await.map_err(row::legacy_error)?;
        let Some(r) = rs.next().await.map_err(row::legacy_error)? else {
            drop(rs);
            tx.rollback().await.map_err(row::legacy_error)?;
            return Ok(false);
        };
        let id: i64 = r.get(0).map_err(row::legacy_error)?;
        let status: String = r.get(1).map_err(row::legacy_error)?;
        let firmware_id: i64 = r.get(2).map_err(row::legacy_error)?;
        drop(rs);
        if u.firmware_update_id.map(i64::from) != Some(firmware_id)
            || !extrittio_backend_core::firmware::ota_transition_allowed(&status, &u.status)
        {
            tx.rollback().await.map_err(row::legacy_error)?;
            return Ok(false);
        }
        let terminal = extrittio_backend_core::firmware::ota_status_is_terminal(&u.status);
        tx.execute("UPDATE ota_deployments SET status=?3,error_message=?4,completed_at=?5 WHERE tenant_id=?1 AND id=?2",params![i.tenant_id_str(),id,u.status,u.error_message,u.completed_at.map(|v|v.and_utc().timestamp_micros())]).await.map_err(row::legacy_error)?;
        if terminal {
            if let Some(shadow) =
                crate::shadows::get_from(&tx, i.tenant_id(), i.device_id()).await?
            {
                if let Some(updated) = extrittio_backend_core::shadows::clear_ota_for_deployment(
                    shadow,
                    id,
                    chrono::Utc::now(),
                )
                .map_err(|error| PersistenceError::CorruptData(error.to_string()))?
                {
                    crate::shadows::store(&tx, i.tenant_id(), &updated).await?;
                }
            }
        }
        tx.commit().await.map_err(row::legacy_error)?;
        Ok(true)
    }
    async fn list_device_deployments(
        &self,
        t: &TenantId,
        device: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Option<OtaDeploymentPage>, PersistenceError> {
        let c = self.connect()?;
        if scalar(
            &c,
            "SELECT count(*) FROM devices WHERE tenant_id=?1 AND id=?2",
            params![t.as_str(), device],
        )
        .await?
            == 0
        {
            return Ok(None);
        }
        let total = scalar(
            &c,
            "SELECT count(*) FROM ota_deployments WHERE tenant_id=?1 AND device_id=?2",
            params![t.as_str(), device],
        )
        .await?;
        let mut rs=c.query("SELECT o.id,o.device_id,o.firmware_update_id,f.version,o.status,o.error_message,o.initiated_at,o.completed_at FROM ota_deployments o JOIN firmware_updates f ON f.id=o.firmware_update_id WHERE o.tenant_id=?1 AND o.device_id=?2 ORDER BY o.initiated_at DESC,o.id DESC LIMIT ?3 OFFSET ?4",params![t.as_str(),device,limit,offset]).await.map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(r) = rs.next().await.map_err(row::legacy_error)? {
            records.push(OtaDeploymentRecord {
                id: row::i32(r.get(0).map_err(row::legacy_error)?, "deployment.id")?,
                device_id: r.get(1).map_err(row::legacy_error)?,
                firmware_update_id: row::i32(r.get(2).map_err(row::legacy_error)?, "firmware.id")?,
                firmware_version: r.get(3).map_err(row::legacy_error)?,
                status: r.get(4).map_err(row::legacy_error)?,
                error_message: r.get(5).map_err(row::legacy_error)?,
                initiated_at: row::datetime(r.get(6).map_err(row::legacy_error)?)?.naive_utc(),
                completed_at: r
                    .get::<Option<i64>>(7)
                    .map_err(row::legacy_error)?
                    .map(row::datetime)
                    .transpose()?
                    .map(|v| v.naive_utc()),
            })
        }
        Ok(Some(OtaDeploymentPage { records, total }))
    }
    async fn trigger_ota(
        &self,
        t: &TenantId,
        device: &str,
        id: i32,
        public_url: &str,
    ) -> Result<TriggerOtaOutcome, PersistenceError> {
        let mut w = self.handles.lock_writer().await;
        let tx = w.transaction().await.map_err(row::legacy_error)?;
        let mut rs = tx
            .query(
                "SELECT id FROM devices WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), device],
            )
            .await
            .map_err(row::legacy_error)?;
        let Some(_device) = rs.next().await.map_err(row::legacy_error)? else {
            tx.rollback().await.map_err(row::legacy_error)?;
            return Ok(TriggerOtaOutcome::DeviceNotFound);
        };
        drop(rs);
        let mut rs=tx.query("SELECT version,url,sha256,blueprint_revision_id FROM firmware_updates WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id]).await.map_err(row::legacy_error)?;
        let Some(f) = rs.next().await.map_err(row::legacy_error)? else {
            tx.rollback().await.map_err(row::legacy_error)?;
            return Ok(TriggerOtaOutcome::FirmwareNotFound);
        };
        let version: String = f.get(0).map_err(row::legacy_error)?;
        let raw_url: String = f.get(1).map_err(row::legacy_error)?;
        let hash: Option<String> = f.get(2).map_err(row::legacy_error)?;
        let target_revision: String = f.get(3).map_err(row::legacy_error)?;
        drop(rs);
        let assigned_revision = {
            let mut rows = tx
                .query(
                    "SELECT contract.blueprint_revision_id
                     FROM device_contract_assignments assignment
                     JOIN device_contracts contract
                       ON contract.tenant_id=assignment.tenant_id
                      AND contract.id=assignment.desired_contract_id
                     WHERE assignment.tenant_id=?1 AND assignment.device_id=?2",
                    params![t.as_str(), device],
                )
                .await
                .map_err(row::legacy_error)?;
            rows.next()
                .await
                .map_err(row::legacy_error)?
                .map(|row| row.get::<String>(0).map_err(row::legacy_error))
                .transpose()?
        };
        if !extrittio_backend_core::firmware::ota_revision_matches(
            assigned_revision.as_deref(),
            &target_revision,
        ) {
            tx.rollback().await.map_err(row::legacy_error)?;
            return Ok(TriggerOtaOutcome::Incompatible);
        }
        let Some(artifact) = extrittio_backend_core::firmware::PreparedOtaArtifact::prepare(
            &version,
            hash.as_deref(),
            &raw_url,
            public_url,
        ) else {
            tx.rollback().await.map_err(row::legacy_error)?;
            return Ok(TriggerOtaOutcome::InvalidArtifact);
        };
        let shadow = crate::shadows::get_from(&tx, t, device)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        let now = chrono::Utc::now().timestamp_micros();
        tx.execute("UPDATE ota_deployments SET status='failed',error_message='Superseded by a new deployment',completed_at=?3 WHERE tenant_id=?1 AND device_id=?2 AND status NOT IN ('success','failed')",params![t.as_str(),device,now]).await.map_err(row::legacy_error)?;
        tx.execute("INSERT INTO ota_deployments(tenant_id,device_id,firmware_update_id,status,initiated_at)VALUES(?1,?2,?3,'pending',?4)",params![t.as_str(),device,id,now]).await.map_err(row::legacy_error)?;
        let deployment_id = scalar(&tx, "SELECT last_insert_rowid()", ()).await?;
        let patch = artifact.desired_patch(id, deployment_id);
        let updated = extrittio_backend_core::shadows::apply_desired_patch(
            shadow,
            &patch,
            chrono::Utc::now(),
        )
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
        crate::shadows::store(&tx, t, &updated).await?;
        tx.commit().await.map_err(row::legacy_error)?;
        Ok(TriggerOtaOutcome::Ready {
            delta: updated.delta,
            version: updated.version,
        })
    }
}
