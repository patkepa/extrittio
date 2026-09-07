use super::*;
use crate::domains::firmware::types::*;
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};
use async_trait::async_trait;
use std::sync::Mutex;

struct RecordingRepository {
    store: FirmwareObjectStore,
    blob: Mutex<Option<NewFirmwareBlobRecord>>,
    fail_create: bool,
}

#[async_trait]
impl FirmwareRepository for RecordingRepository {
    async fn create(
        &self,
        tenant: &TenantId,
        record: NewFirmwareRecord,
        blob: Option<NewFirmwareBlobRecord>,
    ) -> Result<Option<FirmwareRecord>, PersistenceError> {
        assert_eq!(tenant.as_str(), "firmware-test");
        let blob = blob.unwrap();
        // The object must be readable before the database write starts.
        assert_eq!(self.store.get(&blob.storage_key).await.unwrap(), b"abc");
        assert_eq!(
            record.sha256.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        *self.blob.lock().unwrap() = Some(blob.clone());
        if self.fail_create {
            return Err(PersistenceError::Unavailable(
                "injected database failure".into(),
            ));
        }
        Ok(Some(FirmwareRecord {
            id: 1,
            device_type_id: record.device_type_id,
            device_type_name: "sensor".into(),
            version: record.version,
            url: record.url,
            sha256: record.sha256,
            description: record.description,
            created_at: chrono::DateTime::UNIX_EPOCH.naive_utc(),
            file_size: Some(blob.size),
            filename: Some(blob.filename),
            commit_sha: record.commit_sha,
            branch: record.branch,
            ci_run_url: record.ci_run_url,
            build_timestamp: record.build_timestamp,
            changelog: record.changelog,
            source: "manual".into(),
            blueprint_revision_id: record.blueprint_revision_id,
            compatibility: record.compatibility,
            update_strategy: record.update_strategy,
        }))
    }
    async fn delete(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<Option<Option<FirmwareBlobRecord>>, PersistenceError> {
        assert_eq!(tenant.as_str(), "firmware-test");
        assert_eq!(id, 1);
        Ok(self.blob.lock().unwrap().take().map(|blob| {
            Some(FirmwareBlobRecord {
                data: None,
                size: blob.size,
                filename: blob.filename,
                storage_key: Some(blob.storage_key),
                storage_backend: blob.storage_backend,
            })
        }))
    }
    #[allow(unused_variables)]
    async fn ingest_ci(
        &self,
        key_hash: &str,
        params: CiIngestParams,
    ) -> Result<CiIngestOutcome, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn list(
        &self,
        tenant: &TenantId,
        device_type_id: Option<i32>,
        blueprint_revision_id: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<FirmwarePage, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn list_all_deployments(
        &self,
        tenant: &TenantId,
        status: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<GlobalOtaDeploymentPage, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn next_version(
        &self,
        tenant: &TenantId,
        device_type_id: i32,
    ) -> Result<String, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn next_blueprint_version(
        &self,
        tenant: &TenantId,
        blueprint_revision_id: &str,
    ) -> Result<String, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn get_blob(
        &self,
        tenant: &TenantId,
        firmware_update_id: i32,
    ) -> Result<Option<FirmwareBlobRecord>, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn apply_ota_status(
        &self,
        identity: &DeviceIdentity,
        update: OtaStatusUpdate,
    ) -> Result<bool, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn list_device_deployments(
        &self,
        tenant: &TenantId,
        device_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Option<OtaDeploymentPage>, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn trigger_ota(
        &self,
        tenant: &TenantId,
        device_id: &str,
        firmware_update_id: i32,
        public_url: &str,
    ) -> Result<TriggerOtaOutcome, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn next_legacy_blob(&self) -> Result<Option<LegacyFirmwareBlob>, PersistenceError> {
        panic!("unused by firmware storage tests")
    }

    #[allow(unused_variables)]
    async fn mark_blob_migrated(
        &self,
        tenant_id: &str,
        firmware_update_id: i32,
        storage_backend: &str,
        storage_key: &str,
    ) -> Result<bool, PersistenceError> {
        panic!("unused by firmware storage tests")
    }
}
fn context() -> RequestContext {
    RequestContext::from_claims(crate::auth::Claims {
        sub: 1,
        username: "admin".into(),
        role: "admin".into(),
        tenant_id: Some("firmware-test".into()),
        scopes: vec![],
        permission_version: 1,
        auth_epoch: Some("test-epoch".into()),
        exp: 0,
    })
    .unwrap()
}
fn record() -> NewFirmwareRecord {
    PreparedBlueprintFirmware {
        device_type_id: 1,
        compatibility: serde_json::json!({}),
        update_strategy: Some("full".into()),
    }
    .into_record("revision-1".into(), "1.0".into(), String::new(), None, None)
}
#[tokio::test]
async fn upload_failure_removes_the_object_and_preserves_the_database_error() {
    let store = FirmwareObjectStore::in_memory();
    let repository = RecordingRepository {
        store: store.clone(),
        blob: Mutex::new(None),
        fail_create: true,
    };
    let result = upload_blueprint_firmware(
        &context(),
        &repository,
        &store,
        record(),
        "firmware.bin".into(),
        b"abc".to_vec(),
    )
    .await;
    assert!(matches!(
        result,
        Err(AppError::Persistence(PersistenceError::Unavailable(_)))
    ));
    let key = repository
        .blob
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .storage_key
        .clone();
    assert!(store.get(&key).await.is_err());
}
#[tokio::test]
async fn successful_upload_retains_object_until_metadata_deletion() {
    let store = FirmwareObjectStore::in_memory();
    let repository = RecordingRepository {
        store: store.clone(),
        blob: Mutex::new(None),
        fail_create: false,
    };
    let created = upload_blueprint_firmware(
        &context(),
        &repository,
        &store,
        record(),
        "firmware.bin".into(),
        b"abc".to_vec(),
    )
    .await
    .unwrap();
    assert_eq!(created.file_size, Some(3));
    let key = repository
        .blob
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .storage_key
        .clone();
    assert_eq!(store.get(&key).await.unwrap(), b"abc");
    delete_stored_firmware(&context(), &repository, &store, 1)
        .await
        .unwrap();
    assert!(repository.blob.lock().unwrap().is_none());
    assert!(store.get(&key).await.is_err());
}
#[tokio::test]
async fn backend_mismatch_keeps_the_object_but_does_not_fail_metadata_deletion() {
    let store = FirmwareObjectStore::in_memory();
    let repository = RecordingRepository {
        store: store.clone(),
        blob: Mutex::new(None),
        fail_create: false,
    };
    upload_blueprint_firmware(
        &context(),
        &repository,
        &store,
        record(),
        "firmware.bin".into(),
        b"abc".to_vec(),
    )
    .await
    .unwrap();
    let key = {
        let mut blob = repository.blob.lock().unwrap();
        let blob = blob.as_mut().unwrap();
        blob.storage_backend = "different-backend".into();
        blob.storage_key.clone()
    };
    delete_stored_firmware(&context(), &repository, &store, 1)
        .await
        .unwrap();
    assert!(repository.blob.lock().unwrap().is_none());
    assert_eq!(store.get(&key).await.unwrap(), b"abc");
}
