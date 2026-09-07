use async_trait::async_trait;
use turso::{Connection, Row, params};

use crate::domains::firmware::port::FirmwareRepository;
use crate::domains::firmware::types::{
    CiIngestOutcome, CiIngestParams, FirmwareBlobRecord, FirmwarePage, FirmwareRecord,
    GlobalOtaDeploymentPage, GlobalOtaDeploymentRecord, LegacyFirmwareBlob, NewFirmwareBlobRecord,
    NewFirmwareRecord, OtaDeploymentPage, OtaDeploymentRecord, OtaStatusUpdate, TriggerOtaOutcome,
};
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::{TursoAdapter, row};

fn firmware(r: &Row) -> Result<FirmwareRecord, PersistenceError> {
    Ok(FirmwareRecord {
        id: row::i32(r.get(0).map_err(row::error)?, "firmware.id")?,
        device_type_id: row::i32(r.get(1).map_err(row::error)?, "device_type_id")?,
        device_type_name: r.get(2).map_err(row::error)?,
        version: r.get(3).map_err(row::error)?,
        url: r.get(4).map_err(row::error)?,
        sha256: r.get(5).map_err(row::error)?,
        description: r.get(6).map_err(row::error)?,
        created_at: row::datetime(r.get(7).map_err(row::error)?)?.naive_utc(),
        file_size: r
            .get::<Option<i64>>(8)
            .map_err(row::error)?
            .map(|v| row::i32(v, "blob.size"))
            .transpose()?,
        filename: r.get(9).map_err(row::error)?,
        commit_sha: r.get(10).map_err(row::error)?,
        branch: r.get(11).map_err(row::error)?,
        ci_run_url: r.get(12).map_err(row::error)?,
        build_timestamp: r
            .get::<Option<i64>>(13)
            .map_err(row::error)?
            .map(row::datetime)
            .transpose()?
            .map(|v| v.naive_utc()),
        changelog: r.get(14).map_err(row::error)?,
        source: r.get(15).map_err(row::error)?,
        blueprint_revision_id: r.get(16).map_err(row::error)?,
        compatibility: serde_json::from_str::<serde_json::Value>(
            &r.get::<String>(17).map_err(row::error)?,
        )
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        update_strategy: r.get(18).map_err(row::error)?,
    })
}

const FIRMWARE_SELECT: &str = "SELECT f.id,f.device_type_id,dt.name,f.version,f.url,f.sha256,f.description,f.created_at,b.size,b.filename,f.commit_sha,f.branch,f.ci_run_url,f.build_timestamp,f.changelog,f.source,f.blueprint_revision_id,f.compatibility,f.update_strategy FROM firmware_updates f JOIN device_types dt ON dt.tenant_id=f.tenant_id AND dt.id=f.device_type_id LEFT JOIN firmware_blobs b ON b.tenant_id=f.tenant_id AND b.firmware_update_id=f.id";

async fn scalar(
    c: &Connection,
    sql: &str,
    p: impl turso::IntoParams,
) -> Result<i64, PersistenceError> {
    let mut rs = c.query(sql, p).await.map_err(row::error)?;
    rs.next()
        .await
        .map_err(row::error)?
        .ok_or(PersistenceError::NotFound)?
        .get(0)
        .map_err(row::error)
}
fn increment(v: &str) -> String {
    let p: Vec<_> = v.split('.').collect();
    if p.len() == 3
        && let Ok(n) = p[2].parse::<u32>()
    {
        return format!("{}.{}.{}", p[0], p[1], n + 1);
    }
    format!("{v}.1")
}
fn blob(r: &Row) -> Result<FirmwareBlobRecord, PersistenceError> {
    Ok(FirmwareBlobRecord {
        data: r.get(0).map_err(row::error)?,
        size: row::i32(r.get(1).map_err(row::error)?, "blob.size")?,
        filename: r.get(2).map_err(row::error)?,
        storage_key: r.get(3).map_err(row::error)?,
        storage_backend: r.get(4).map_err(row::error)?,
    })
}

async fn insert_firmware(
    c: &Connection,
    tenant: &str,
    r: NewFirmwareRecord,
) -> Result<i32, PersistenceError> {
    let compatibility = serde_json::to_string(&r.compatibility)
        .map_err(|error| PersistenceError::Internal(error.to_string()))?;
    c.execute("INSERT INTO firmware_updates(tenant_id,device_type_id,version,url,description,sha256,commit_sha,branch,ci_run_url,build_timestamp,changelog,source,created_at,blueprint_revision_id,compatibility,update_strategy)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",params![tenant,r.device_type_id,r.version,r.url,r.description,r.sha256,r.commit_sha,r.branch,r.ci_run_url,r.build_timestamp.map(|v|v.and_utc().timestamp_micros()),r.changelog,r.source.unwrap_or_else(||"manual".into()),chrono::Utc::now().timestamp_micros(),r.blueprint_revision_id,compatibility,r.update_strategy]).await.map_err(row::error)?;
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
        .map_err(row::error)?;
    rs.next()
        .await
        .map_err(row::error)?
        .map(|r| firmware(&r))
        .transpose()
}

#[async_trait]
impl FirmwareRepository for TursoAdapter {
    async fn ingest_ci(
        &self,
        key_hash: &str,
        p: CiIngestParams,
    ) -> Result<CiIngestOutcome, PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        let mut rs = tx
            .query(
                "SELECT tenant_id,device_type_id FROM api_keys WHERE key_hash=?1",
                params![key_hash],
            )
            .await
            .map_err(row::error)?;
        let Some(k) = rs.next().await.map_err(row::error)? else {
            tx.rollback().await.map_err(row::error)?;
            return Ok(CiIngestOutcome::Unauthorized);
        };
        let tenant: String = k.get(0).map_err(row::error)?;
        let scope: Option<i64> = k.get(1).map_err(row::error)?;
        drop(rs);
        tx.execute(
            "UPDATE api_keys SET last_used_at=?2 WHERE key_hash=?1",
            params![key_hash, chrono::Utc::now().timestamp_micros()],
        )
        .await
        .map_err(row::error)?;
        let mut rs = tx
            .query(
                "SELECT id,name FROM device_types WHERE tenant_id=?1 AND name=?2",
                params![tenant.clone(), p.device_type_name],
            )
            .await
            .map_err(row::error)?;
        let Some(dt) = rs.next().await.map_err(row::error)? else {
            tx.commit().await.map_err(row::error)?;
            return Ok(CiIngestOutcome::DeviceTypeNotFound);
        };
        let dt_id = row::i32(dt.get(0).map_err(row::error)?, "device_type.id")?;
        let dt_name = dt.get(1).map_err(row::error)?;
        drop(rs);
        if let Some(scope) = scope
            && row::i32(scope, "api_key.device_type_id")? != dt_id
        {
            tx.commit().await.map_err(row::error)?;
            return Ok(CiIngestOutcome::Forbidden {
                scoped_device_type_id: row::i32(scope, "api_key.device_type_id")?,
            });
        }
        let version = p.version.clone();
        let id = insert_firmware(
            &tx,
            &tenant,
            NewFirmwareRecord {
                device_type_id: dt_id,
                version: p.version,
                url: p.artifact_url,
                sha256: p.sha256,
                description: p.description,
                commit_sha: p.commit_sha,
                branch: p.branch,
                ci_run_url: p.ci_run_url,
                build_timestamp: p.build_timestamp,
                changelog: p.changelog,
                source: Some("ci".into()),
                blueprint_revision_id: None,
                compatibility: serde_json::json!({}),
                update_strategy: None,
            },
        )
        .await?;
        tx.commit().await.map_err(row::error)?;
        Ok(CiIngestOutcome::Created {
            firmware_id: id,
            version,
            device_type_name: dt_name,
        })
    }
    async fn list(
        &self,
        t: &TenantId,
        dt: Option<i32>,
        blueprint_revision_id: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<FirmwarePage, PersistenceError> {
        let c = self.database.connect()?;
        let total=scalar(&c,"SELECT count(*) FROM firmware_updates WHERE tenant_id=?1 AND (?2 IS NULL OR device_type_id=?2) AND (?3 IS NULL OR blueprint_revision_id=?3)",params![t.as_str(),dt,blueprint_revision_id.clone()]).await?;
        let mut rs=c.query(&format!("{FIRMWARE_SELECT} WHERE f.tenant_id=?1 AND (?2 IS NULL OR f.device_type_id=?2) AND (?3 IS NULL OR f.blueprint_revision_id=?3) ORDER BY f.created_at DESC,f.id DESC LIMIT ?4 OFFSET ?5"),params![t.as_str(),dt,blueprint_revision_id,limit,offset]).await.map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(r) = rs.next().await.map_err(row::error)? {
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
        let c = self.database.connect()?;
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
        let select = "SELECT o.id,d.id,d.name,d.status,d.firmware,dt.id,dt.name,fl.id,fl.name,o.firmware_update_id,f.version,o.status,o.error_message,o.initiated_at,o.completed_at FROM ota_deployments o JOIN firmware_updates f ON f.id=o.firmware_update_id JOIN devices d ON d.tenant_id=o.tenant_id AND d.id=o.device_id JOIN device_types dt ON dt.tenant_id=d.tenant_id AND dt.id=d.device_type_id LEFT JOIN fleets fl ON fl.tenant_id=d.tenant_id AND fl.id=d.fleet_id";
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
        .map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(r) = rs.next().await.map_err(row::error)? {
            records.push(GlobalOtaDeploymentRecord {
                id: row::i32(r.get(0).map_err(row::error)?, "deployment.id")?,
                device_id: r.get(1).map_err(row::error)?,
                device_name: r.get(2).map_err(row::error)?,
                device_status: r.get(3).map_err(row::error)?,
                current_firmware: r.get(4).map_err(row::error)?,
                device_type_id: row::i32(r.get(5).map_err(row::error)?, "device_type.id")?,
                device_type_name: r.get(6).map_err(row::error)?,
                fleet_id: r
                    .get::<Option<i64>>(7)
                    .map_err(row::error)?
                    .map(|v| row::i32(v, "fleet.id"))
                    .transpose()?,
                fleet_name: r.get(8).map_err(row::error)?,
                firmware_update_id: row::i32(r.get(9).map_err(row::error)?, "firmware.id")?,
                firmware_version: r.get(10).map_err(row::error)?,
                status: r.get(11).map_err(row::error)?,
                error_message: r.get(12).map_err(row::error)?,
                initiated_at: row::datetime(r.get(13).map_err(row::error)?)?.naive_utc(),
                completed_at: r
                    .get::<Option<i64>>(14)
                    .map_err(row::error)?
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
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        if scalar(
            &tx,
            "SELECT count(*) FROM device_types WHERE tenant_id=?1 AND id=?2",
            params![t.as_str(), r.device_type_id],
        )
        .await?
            == 0
        {
            tx.rollback().await.map_err(row::error)?;
            return Ok(None);
        }
        let id = insert_firmware(&tx, t.as_str(), r).await?;
        if let Some(b) = b {
            tx.execute("INSERT INTO firmware_blobs(firmware_update_id,tenant_id,data,size,filename,storage_key,storage_backend,created_at)VALUES(?1,?2,NULL,?3,?4,?5,?6,?7)",params![id,t.as_str(),b.size,b.filename,b.storage_key,b.storage_backend,chrono::Utc::now().timestamp_micros()]).await.map_err(row::error)?;
            tx.execute(
                "UPDATE firmware_updates SET url=?3 WHERE tenant_id=?1 AND id=?2",
                params![
                    t.as_str(),
                    id,
                    format!("/api/v1/firmware-updates/{id}/download")
                ],
            )
            .await
            .map_err(row::error)?;
        }
        let out = find_firmware(&tx, t.as_str(), id).await?;
        tx.commit().await.map_err(row::error)?;
        Ok(out)
    }
    async fn next_version(&self, t: &TenantId, dt: i32) -> Result<String, PersistenceError> {
        let c = self.database.connect()?;
        let mut rs=c.query("SELECT version FROM firmware_updates WHERE tenant_id=?1 AND device_type_id=?2 ORDER BY created_at DESC,id DESC LIMIT 1",params![t.as_str(),dt]).await.map_err(row::error)?;
        Ok(rs
            .next()
            .await
            .map_err(row::error)?
            .map(|r| r.get::<String>(0).map_err(row::error))
            .transpose()?
            .map_or_else(|| "1.0.0".into(), |v| increment(&v)))
    }
    async fn next_blueprint_version(
        &self,
        t: &TenantId,
        blueprint_revision_id: &str,
    ) -> Result<String, PersistenceError> {
        let c = self.database.connect()?;
        let mut rs=c.query("SELECT version FROM firmware_updates WHERE tenant_id=?1 AND blueprint_revision_id=?2 ORDER BY created_at DESC,id DESC LIMIT 1",params![t.as_str(),blueprint_revision_id]).await.map_err(row::error)?;
        Ok(rs
            .next()
            .await
            .map_err(row::error)?
            .map(|r| r.get::<String>(0).map_err(row::error))
            .transpose()?
            .map_or_else(|| "1.0.0".into(), |v| increment(&v)))
    }
    async fn get_blob(
        &self,
        t: &TenantId,
        id: i32,
    ) -> Result<Option<FirmwareBlobRecord>, PersistenceError> {
        let c = self.database.connect()?;
        let mut rs=c.query("SELECT data,size,filename,storage_key,storage_backend FROM firmware_blobs WHERE tenant_id=?1 AND firmware_update_id=?2",params![t.as_str(),id]).await.map_err(row::error)?;
        rs.next()
            .await
            .map_err(row::error)?
            .map(|r| blob(&r))
            .transpose()
    }
    async fn delete(
        &self,
        t: &TenantId,
        id: i32,
    ) -> Result<Option<Option<FirmwareBlobRecord>>, PersistenceError> {
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        let mut rs=tx.query("SELECT data,size,filename,storage_key,storage_backend FROM firmware_blobs WHERE tenant_id=?1 AND firmware_update_id=?2",params![t.as_str(),id]).await.map_err(row::error)?;
        let b = rs
            .next()
            .await
            .map_err(row::error)?
            .map(|r| blob(&r))
            .transpose()?;
        drop(rs);
        let n = tx
            .execute(
                "DELETE FROM firmware_updates WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), id],
            )
            .await
            .map_err(row::error)?;
        tx.commit().await.map_err(row::error)?;
        Ok((n > 0).then_some(b))
    }
    async fn apply_ota_status(
        &self,
        i: &DeviceIdentity,
        u: OtaStatusUpdate,
    ) -> Result<bool, PersistenceError> {
        let w = self.database.writer().await;
        let mut rs=w.query("SELECT id FROM ota_deployments WHERE tenant_id=?1 AND device_id=?2 AND status NOT IN ('success','failed') AND (?3 IS NULL OR firmware_update_id=?3) ORDER BY initiated_at DESC,id DESC LIMIT 1",params![i.tenant_id_str(),i.device_id(),u.firmware_update_id]).await.map_err(row::error)?;
        let Some(r) = rs.next().await.map_err(row::error)? else {
            return Ok(false);
        };
        let id: i64 = r.get(0).map_err(row::error)?;
        drop(rs);
        w.execute("UPDATE ota_deployments SET status=?3,error_message=?4,completed_at=?5 WHERE tenant_id=?1 AND id=?2",params![i.tenant_id_str(),id,u.status,u.error_message,u.completed_at.map(|v|v.and_utc().timestamp_micros())]).await.map(|n|n==1).map_err(row::error)
    }
    async fn list_device_deployments(
        &self,
        t: &TenantId,
        device: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Option<OtaDeploymentPage>, PersistenceError> {
        let c = self.database.connect()?;
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
        let mut rs=c.query("SELECT o.id,o.device_id,o.firmware_update_id,f.version,o.status,o.error_message,o.initiated_at,o.completed_at FROM ota_deployments o JOIN firmware_updates f ON f.id=o.firmware_update_id WHERE o.tenant_id=?1 AND o.device_id=?2 ORDER BY o.initiated_at DESC,o.id DESC LIMIT ?3 OFFSET ?4",params![t.as_str(),device,limit,offset]).await.map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(r) = rs.next().await.map_err(row::error)? {
            records.push(OtaDeploymentRecord {
                id: row::i32(r.get(0).map_err(row::error)?, "deployment.id")?,
                device_id: r.get(1).map_err(row::error)?,
                firmware_update_id: row::i32(r.get(2).map_err(row::error)?, "firmware.id")?,
                firmware_version: r.get(3).map_err(row::error)?,
                status: r.get(4).map_err(row::error)?,
                error_message: r.get(5).map_err(row::error)?,
                initiated_at: row::datetime(r.get(6).map_err(row::error)?)?.naive_utc(),
                completed_at: r
                    .get::<Option<i64>>(7)
                    .map_err(row::error)?
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
        let mut w = self.database.writer().await;
        let tx = w.transaction().await.map_err(row::error)?;
        let mut rs = tx
            .query(
                "SELECT device_type_id FROM devices WHERE tenant_id=?1 AND id=?2",
                params![t.as_str(), device],
            )
            .await
            .map_err(row::error)?;
        let Some(d) = rs.next().await.map_err(row::error)? else {
            tx.rollback().await.map_err(row::error)?;
            return Ok(TriggerOtaOutcome::DeviceNotFound);
        };
        let dt: i64 = d.get(0).map_err(row::error)?;
        drop(rs);
        let mut rs=tx.query("SELECT device_type_id,version,url,sha256,blueprint_revision_id FROM firmware_updates WHERE tenant_id=?1 AND id=?2",params![t.as_str(),id]).await.map_err(row::error)?;
        let Some(f) = rs.next().await.map_err(row::error)? else {
            tx.rollback().await.map_err(row::error)?;
            return Ok(TriggerOtaOutcome::FirmwareNotFound);
        };
        let fdt: i64 = f.get(0).map_err(row::error)?;
        let version: String = f.get(1).map_err(row::error)?;
        let raw_url: String = f.get(2).map_err(row::error)?;
        let hash: Option<String> = f.get(3).map_err(row::error)?;
        let target_revision: Option<String> = f.get(4).map_err(row::error)?;
        drop(rs);
        let compatible = if let Some(target_revision) = target_revision {
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
                .map_err(row::error)?;
            rows.next()
                .await
                .map_err(row::error)?
                .map(|row| row.get::<String>(0).map_err(row::error))
                .transpose()?
                .is_some_and(|revision| revision == target_revision)
        } else {
            dt == fdt
        };
        if !compatible {
            tx.rollback().await.map_err(row::error)?;
            return Ok(TriggerOtaOutcome::Incompatible);
        }
        if !crate::domains::firmware::types::valid_ota_artifact(
            &version,
            hash.as_deref(),
            if raw_url.starts_with("https://") {
                &raw_url
            } else {
                public_url
            },
        ) {
            tx.rollback().await.map_err(row::error)?;
            return Ok(TriggerOtaOutcome::InvalidArtifact);
        }
        let mut rs=tx.query("SELECT desired,reported,version FROM device_shadows WHERE tenant_id=?1 AND device_id=?2",params![t.as_str(),device]).await.map_err(row::error)?;
        let s = rs
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?;
        let desired_raw: String = s.get(0).map_err(row::error)?;
        let reported_raw: String = s.get(1).map_err(row::error)?;
        let shadow_version: i64 = s.get(2).map_err(row::error)?;
        drop(rs);
        use extrittio_common::ota::fields;
        let url = if raw_url.starts_with("https://") {
            raw_url
        } else {
            public_url.to_string()
        };
        let mut ota = serde_json::json!({fields::FIRMWARE_VERSION:version,fields::FIRMWARE_URL:url,fields::FIRMWARE_UPDATE_ID:id});
        if let Some(v) = hash {
            ota[fields::SHA256] = serde_json::Value::String(v)
        }
        let mut patch = serde_json::Map::new();
        patch.insert(fields::SHADOW_KEY.into(), ota);
        let desired: serde_json::Value = serde_json::from_str(&desired_raw)
            .map_err(|e| PersistenceError::CorruptData(e.to_string()))?;
        let reported: serde_json::Value = serde_json::from_str(&reported_raw)
            .map_err(|e| PersistenceError::CorruptData(e.to_string()))?;
        let desired = extrittio_common::shadow::merge_json(
            if desired.is_object() {
                desired
            } else {
                serde_json::json!({})
            },
            &patch,
        );
        let delta = extrittio_common::shadow::compute_delta(
            &desired,
            &if reported.is_object() {
                reported
            } else {
                serde_json::json!({})
            },
        );
        let next = shadow_version
            .checked_add(1)
            .ok_or_else(|| PersistenceError::Internal("shadow version overflow".into()))?;
        let now = chrono::Utc::now().timestamp_micros();
        tx.execute("UPDATE device_shadows SET desired=?3,delta=?4,version=?5,updated_at=?6 WHERE tenant_id=?1 AND device_id=?2",params![t.as_str(),device,serde_json::to_string(&desired).map_err(|e|PersistenceError::Internal(e.to_string()))?,serde_json::to_string(&delta).map_err(|e|PersistenceError::Internal(e.to_string()))?,next,now]).await.map_err(row::error)?;
        tx.execute("INSERT INTO ota_deployments(tenant_id,device_id,firmware_update_id,status,initiated_at)VALUES(?1,?2,?3,'pending',?4)",params![t.as_str(),device,id,now]).await.map_err(row::error)?;
        tx.commit().await.map_err(row::error)?;
        Ok(TriggerOtaOutcome::Ready {
            delta,
            version: row::i32(next, "shadow.version")?,
        })
    }
    async fn next_legacy_blob(&self) -> Result<Option<LegacyFirmwareBlob>, PersistenceError> {
        let c = self.database.connect()?;
        let mut rs=c.query("SELECT tenant_id,firmware_update_id,filename,data FROM firmware_blobs WHERE storage_backend='database' AND data IS NOT NULL ORDER BY firmware_update_id LIMIT 1",()).await.map_err(row::error)?;
        rs.next()
            .await
            .map_err(row::error)?
            .map(|r| {
                Ok(LegacyFirmwareBlob {
                    tenant_id: r.get(0).map_err(row::error)?,
                    firmware_update_id: row::i32(r.get(1).map_err(row::error)?, "firmware.id")?,
                    filename: r.get(2).map_err(row::error)?,
                    data: r.get(3).map_err(row::error)?,
                })
            })
            .transpose()
    }
    async fn mark_blob_migrated(
        &self,
        t: &str,
        id: i32,
        backend: &str,
        key: &str,
    ) -> Result<bool, PersistenceError> {
        self.database.writer().await.execute("UPDATE firmware_blobs SET data=NULL,storage_backend=?3,storage_key=?4 WHERE tenant_id=?1 AND firmware_update_id=?2 AND storage_backend='database' AND data IS NOT NULL",params![t,id,backend,key]).await.map(|n|n>0).map_err(row::error)
    }
}
