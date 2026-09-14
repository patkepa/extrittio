use async_trait::async_trait;
use extrittio_backend_core::{
    CiIngestOutcome, CiIngestParams, CiIngestRepository, PersistenceError, authorize_ci_device_type,
};
use turso::params;

use crate::row::legacy_error as map_error;
use crate::{TursoConnectionHandles, row};

#[derive(Clone)]
pub struct TursoCiIngestRepository {
    handles: TursoConnectionHandles,
}

impl TursoCiIngestRepository {
    #[must_use]
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
}

#[async_trait]
impl CiIngestRepository for TursoCiIngestRepository {
    async fn ingest_ci(
        &self,
        key_hash: &str,
        p: CiIngestParams,
    ) -> Result<CiIngestOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let tx = writer.transaction().await.map_err(map_error)?;
        let mut rows = tx
            .query(
                "SELECT tenant_id,device_type_id FROM api_keys WHERE key_hash=?1",
                params![key_hash],
            )
            .await
            .map_err(map_error)?;
        let Some(key) = rows.next().await.map_err(map_error)? else {
            tx.rollback().await.map_err(map_error)?;
            return Ok(CiIngestOutcome::Unauthorized);
        };
        let tenant: String = key.get(0).map_err(map_error)?;
        let scope: Option<i64> = key.get(1).map_err(map_error)?;
        drop(rows);
        tx.execute(
            "UPDATE api_keys SET last_used_at=?2 WHERE key_hash=?1",
            params![key_hash, chrono::Utc::now().timestamp_micros()],
        )
        .await
        .map_err(map_error)?;
        let mut rows = tx
            .query(
                "SELECT id,name FROM device_types WHERE tenant_id=?1 AND name=?2",
                params![tenant.clone(), p.device_type_name],
            )
            .await
            .map_err(map_error)?;
        let Some(device_type) = rows.next().await.map_err(map_error)? else {
            tx.commit().await.map_err(map_error)?;
            return Ok(CiIngestOutcome::DeviceTypeNotFound);
        };
        let device_type_id = row::i32(device_type.get(0).map_err(map_error)?, "device_type.id")?;
        let device_type_name = device_type.get(1).map_err(map_error)?;
        drop(rows);
        let scope = scope
            .map(|id| row::i32(id, "api_key.device_type_id"))
            .transpose()?;
        if let Err(outcome) = authorize_ci_device_type(scope, device_type_id) {
            tx.commit().await.map_err(map_error)?;
            return Ok(outcome);
        }
        let version = p.version.clone();
        tx.execute(
            "INSERT INTO firmware_updates(tenant_id,device_type_id,version,url,description,sha256,commit_sha,branch,ci_run_url,build_timestamp,changelog,source,created_at,blueprint_revision_id,compatibility,update_strategy)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![tenant, device_type_id, p.version, p.artifact_url, p.description, p.sha256,
                p.commit_sha, p.branch, p.ci_run_url, p.build_timestamp.map(|v| v.timestamp_micros()),
                p.changelog, "ci", chrono::Utc::now().timestamp_micros(),
                Option::<String>::None, "{}", Option::<String>::None],
        ).await.map_err(map_error)?;
        let mut rows = tx
            .query("SELECT last_insert_rowid()", ())
            .await
            .map_err(map_error)?;
        let firmware_id = row::i32(
            rows.next()
                .await
                .map_err(map_error)?
                .ok_or(PersistenceError::NotFound)?
                .get(0)
                .map_err(map_error)?,
            "firmware.id",
        )?;
        drop(rows);
        tx.commit().await.map_err(map_error)?;
        Ok(CiIngestOutcome::Created {
            firmware_id,
            version,
            device_type_name,
        })
    }
}
