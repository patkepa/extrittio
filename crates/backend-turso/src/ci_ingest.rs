use async_trait::async_trait;
use extrittio_backend_core::{
    CiIngestOutcome, CiIngestParams, CiIngestRepository, PersistenceError, authorize_ci_blueprint,
};
use turso::params;

use crate::row::legacy_error as map_error;
use crate::{TursoConnectionHandles, row};

#[cfg(test)]
mod blueprint_tests {
    use super::*;
    use extrittio_backend_core::{ApiKeyRepository, CreateApiKeyRecord, TenantId};

    fn request(revision: &str, version: &str) -> CiIngestParams {
        CiIngestParams {
            blueprint_revision_id: revision.into(),
            version: version.into(),
            artifact_url: "https://example.test/fw.bin".into(),
            sha256: Some("a".repeat(64)),
            commit_sha: None,
            branch: None,
            ci_run_url: None,
            build_timestamp: None,
            description: None,
            changelog: None,
        }
    }

    #[tokio::test]
    async fn keys_and_ci_use_blueprint_scope_without_device_types() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("ci.db"),
            std::time::Duration::from_secs(1),
        )
        .await
        .unwrap();
        let connection = database.shared_handles().connect().unwrap();
        connection.execute_batch(
            "CREATE TABLE device_blueprints (tenant_id TEXT, id TEXT, name TEXT);
             CREATE TABLE device_blueprint_revisions (tenant_id TEXT, id TEXT, blueprint_id TEXT, document TEXT);
             CREATE TABLE api_keys (id INTEGER PRIMARY KEY, tenant_id TEXT, name TEXT,
                key_hash TEXT UNIQUE, key_prefix TEXT, blueprint_id TEXT, created_at INTEGER, last_used_at INTEGER);
             CREATE TABLE firmware_updates (id INTEGER PRIMARY KEY, tenant_id TEXT, version TEXT,
                url TEXT, description TEXT, sha256 TEXT, commit_sha TEXT, branch TEXT,
                ci_run_url TEXT, build_timestamp INTEGER, changelog TEXT, source TEXT,
                created_at INTEGER, blueprint_revision_id TEXT, compatibility TEXT, update_strategy TEXT,
                UNIQUE(tenant_id,blueprint_revision_id,version));
             INSERT INTO device_blueprints VALUES ('tenant','blueprint','Sensor');
             INSERT INTO device_blueprints VALUES ('other','foreign','Foreign');"
        ).await.unwrap();
        let document = include_str!("../../../blueprints/environment-sensor.create-request.json");
        for (tenant, revision, blueprint) in [
            ("tenant", "revision-1", "blueprint"),
            ("tenant", "revision-2", "blueprint"),
            ("tenant", "mismatch", "different"),
            ("other", "foreign-revision", "foreign"),
        ] {
            connection
                .execute(
                    "INSERT INTO device_blueprint_revisions VALUES (?1,?2,?3,?4)",
                    params![tenant, revision, blueprint, document],
                )
                .await
                .unwrap();
        }
        let tenant = TenantId::new("tenant").unwrap();
        let keys = crate::api_keys::TursoApiKeyRepository::from_handles(database.shared_handles());
        let key_record = |scope: Option<&str>, hash: &str| CreateApiKeyRecord {
            name: "CI".into(),
            key_hash: hash.into(),
            key_prefix: "prefix".into(),
            blueprint_id: scope.map(str::to_owned),
        };
        assert!(
            keys.create(&tenant, key_record(Some("foreign"), "invalid"))
                .await
                .is_err()
        );
        let key = keys
            .create(&tenant, key_record(Some("blueprint"), "scoped"))
            .await
            .unwrap();
        let listed = keys.list(&tenant).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].key.blueprint_id.as_deref(), Some("blueprint"));
        assert_eq!(listed[0].blueprint_name.as_deref(), Some("Sensor"));
        connection.execute_batch(
            "CREATE TABLE firmware_blobs (tenant_id TEXT, firmware_update_id INTEGER, size INTEGER, filename TEXT);"
        ).await.unwrap();
        use extrittio_backend_core::firmware::{FirmwareRepository, NewFirmwareRecord};
        let firmware =
            crate::firmware::TursoFirmwareRepository::from_handles(database.shared_handles());
        let manual = |revision: &str| NewFirmwareRecord {
            version: "manual-1".into(),
            url: "https://example.test/fw.bin".into(),
            sha256: Some("a".repeat(64)),
            description: None,
            commit_sha: None,
            branch: None,
            ci_run_url: None,
            build_timestamp: None,
            changelog: None,
            source: None,
            blueprint_revision_id: revision.into(),
            compatibility: serde_json::json!({}),
            update_strategy: Some("partition_swap".into()),
        };
        assert!(
            firmware
                .create(&tenant, manual("foreign-revision"), None)
                .await
                .unwrap()
                .is_none()
        );
        let created = firmware
            .create(&tenant, manual("revision-1"), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(created.blueprint_revision_id, "revision-1");
        assert_eq!(created.source, "manual");
        let ci = TursoCiIngestRepository::from_handles(database.shared_handles());
        assert!(matches!(
            ci.ingest_ci("unknown", request("revision-1", "1"))
                .await
                .unwrap(),
            CiIngestOutcome::Unauthorized
        ));
        assert!(matches!(
            ci.ingest_ci("scoped", request("foreign-revision", "1"))
                .await
                .unwrap(),
            CiIngestOutcome::BlueprintRevisionNotFound
        ));
        assert!(matches!(
            ci.ingest_ci("scoped", request("mismatch", "1"))
                .await
                .unwrap(),
            CiIngestOutcome::Forbidden { .. }
        ));
        assert!(
            keys.list(&tenant).await.unwrap()[0]
                .key
                .last_used_at
                .is_none()
        );
        for revision in ["revision-1", "revision-2"] {
            assert!(matches!(
                ci.ingest_ci("scoped", request(revision, "1"))
                    .await
                    .unwrap(),
                CiIngestOutcome::Created { .. }
            ));
        }
        assert!(
            ci.ingest_ci("scoped", request("revision-1", "1"))
                .await
                .is_err()
        );
        assert!(
            keys.list(&tenant).await.unwrap()[0]
                .key
                .last_used_at
                .is_some()
        );
        let mut rows = connection.query("SELECT blueprint_revision_id,compatibility,update_strategy FROM firmware_updates WHERE source='ci' ORDER BY id", ()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "revision-1");
        let compatibility: serde_json::Value =
            serde_json::from_str(&row.get::<String>(1).unwrap()).unwrap();
        assert_eq!(compatibility, serde_json::json!({ "contractApi": 1 }));
        assert_eq!(row.get::<String>(2).unwrap(), "binary_replacement");
        drop(rows);
        keys.create(&tenant, key_record(None, "tenant-wide"))
            .await
            .unwrap();
        assert!(matches!(
            ci.ingest_ci("tenant-wide", request("mismatch", "1"))
                .await
                .unwrap(),
            CiIngestOutcome::Created { .. }
        ));
        assert!(matches!(
            ci.ingest_ci("tenant-wide", request("foreign-revision", "1"))
                .await
                .unwrap(),
            CiIngestOutcome::BlueprintRevisionNotFound
        ));
        assert!(
            !keys
                .delete(&TenantId::new("other").unwrap(), key.id)
                .await
                .unwrap()
        );
        assert!(keys.delete(&tenant, key.id).await.unwrap());
        assert!(matches!(
            ci.ingest_ci("scoped", request("revision-1", "2"))
                .await
                .unwrap(),
            CiIngestOutcome::Unauthorized
        ));
    }
}

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
                "SELECT tenant_id,blueprint_id FROM api_keys WHERE key_hash=?1",
                params![key_hash],
            )
            .await
            .map_err(map_error)?;
        let Some(key) = rows.next().await.map_err(map_error)? else {
            tx.rollback().await.map_err(map_error)?;
            return Ok(CiIngestOutcome::Unauthorized);
        };
        let tenant: String = key.get(0).map_err(map_error)?;
        let scope: Option<String> = key.get(1).map_err(map_error)?;
        drop(rows);
        let mut rows = tx.query(
            "SELECT blueprint_id,document FROM device_blueprint_revisions WHERE tenant_id=?1 AND id=?2",
            params![tenant.clone(), p.blueprint_revision_id.clone()],
        ).await.map_err(map_error)?;
        let Some(revision) = rows.next().await.map_err(map_error)? else {
            tx.rollback().await.map_err(map_error)?;
            return Ok(CiIngestOutcome::BlueprintRevisionNotFound);
        };
        let blueprint_id: String = revision.get(0).map_err(map_error)?;
        let document: String = revision.get(1).map_err(map_error)?;
        drop(rows);
        if let Err(outcome) = authorize_ci_blueprint(scope.as_deref(), &blueprint_id) {
            tx.rollback().await.map_err(map_error)?;
            return Ok(outcome);
        }
        let document = serde_json::from_str(&document)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
        let Some((compatibility, update_strategy)) =
            extrittio_backend_core::ci_ingest::ci_firmware_metadata(document)?
        else {
            tx.rollback().await.map_err(map_error)?;
            return Ok(CiIngestOutcome::UnsupportedFirmware);
        };
        let revision_id = p.blueprint_revision_id.clone();
        let version = p.version.clone();
        tx.execute(
            "INSERT INTO firmware_updates(tenant_id,version,url,description,sha256,commit_sha,branch,ci_run_url,build_timestamp,changelog,source,created_at,blueprint_revision_id,compatibility,update_strategy)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![tenant, p.version, p.artifact_url, p.description, p.sha256,
                p.commit_sha, p.branch, p.ci_run_url, p.build_timestamp.map(|v| v.timestamp_micros()),
                p.changelog, "ci", chrono::Utc::now().timestamp_micros(),
                p.blueprint_revision_id, compatibility.to_string(), update_strategy],
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
        tx.execute(
            "UPDATE api_keys SET last_used_at=?2 WHERE key_hash=?1",
            params![key_hash, chrono::Utc::now().timestamp_micros()],
        )
        .await
        .map_err(map_error)?;
        tx.commit().await.map_err(map_error)?;
        Ok(CiIngestOutcome::Created {
            firmware_id,
            version,
            blueprint_revision_id: revision_id,
        })
    }
}
