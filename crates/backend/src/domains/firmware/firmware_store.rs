use std::fmt;
use std::sync::Arc;

use anyhow::{Context, bail};
#[cfg(feature = "s3")]
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use uuid::Uuid;

use crate::config::FirmwareStorageConfig;
use crate::state::ReadinessRegistry;

#[derive(Clone)]
pub struct FirmwareObjectStore {
    inner: Arc<dyn ObjectStore>,
    backend: &'static str,
}

impl fmt::Debug for FirmwareObjectStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FirmwareObjectStore")
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

impl FirmwareObjectStore {
    #[doc(hidden)]
    #[must_use]
    pub fn in_memory() -> Self {
        Self::new(Arc::new(object_store::memory::InMemory::new()), "memory")
    }

    pub fn from_config(config: &FirmwareStorageConfig) -> anyhow::Result<Self> {
        match config {
            FirmwareStorageConfig::Local { path } => {
                std::fs::create_dir_all(path).with_context(|| {
                    format!(
                        "failed to create firmware storage directory {}",
                        path.display()
                    )
                })?;
                let canonical_path = path.canonicalize().with_context(|| {
                    format!(
                        "failed to resolve firmware storage directory {}",
                        path.display()
                    )
                })?;
                let store = LocalFileSystem::new_with_prefix(&canonical_path)
                    .context("failed to initialize local firmware object store")?;
                Ok(Self::new(Arc::new(store), "local"))
            }
            #[cfg(feature = "s3")]
            FirmwareStorageConfig::S3 {
                bucket,
                region,
                endpoint,
                allow_http,
                virtual_hosted_style,
            } => {
                let mut builder = AmazonS3Builder::from_env()
                    .with_bucket_name(bucket)
                    .with_region(region)
                    .with_allow_http(*allow_http)
                    .with_virtual_hosted_style_request(*virtual_hosted_style)
                    .with_disable_bulk_delete(true);
                if let Some(endpoint) = endpoint {
                    builder = builder.with_endpoint(endpoint);
                }
                let store = builder
                    .build()
                    .context("failed to initialize S3 firmware object store")?;
                Ok(Self::new(Arc::new(store), "s3"))
            }
        }
    }

    fn new(inner: Arc<dyn ObjectStore>, backend: &'static str) -> Self {
        Self { inner, backend }
    }

    #[must_use]
    pub fn backend(&self) -> &'static str {
        self.backend
    }

    #[must_use]
    pub fn allocate_key(&self, tenant_id: &str, filename: &str) -> String {
        let tenant = safe_segment(tenant_id, "tenant");
        let filename = safe_segment(filename, "firmware.bin");
        format!("tenants/{tenant}/firmware/{}/{filename}", Uuid::new_v4())
    }

    pub async fn verify(&self) -> anyhow::Result<()> {
        let key = format!(".extrittio-health/{}", Uuid::new_v4());
        self.put(&key, Vec::new()).await?;
        if let Err(error) = self.delete(&key).await {
            tracing::error!(%error, "Firmware storage probe cleanup failed");
            return Err(error);
        }
        Ok(())
    }

    pub async fn put(&self, key: &str, data: Vec<u8>) -> anyhow::Result<()> {
        let path = object_path(key)?;
        self.inner
            .put(&path, PutPayload::from(data))
            .await
            .with_context(|| format!("failed to store firmware object {key}"))?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let path = object_path(key)?;
        let result = self
            .inner
            .get(&path)
            .await
            .with_context(|| format!("failed to read firmware object {key}"))?;
        Ok(result
            .bytes()
            .await
            .with_context(|| format!("failed to buffer firmware object {key}"))?
            .to_vec())
    }

    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let path = object_path(key)?;
        self.inner
            .delete(&path)
            .await
            .with_context(|| format!("failed to delete firmware object {key}"))
    }
}

pub async fn run_readiness_monitor(store: FirmwareObjectStore, readiness: Arc<ReadinessRegistry>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        match store.verify().await {
            Ok(()) => readiness.set_worker("firmware-object-store", true),
            Err(error) => {
                readiness.set_worker("firmware-object-store", false);
                tracing::error!(%error, "Firmware object storage readiness probe failed");
            }
        }
    }
}

fn object_path(key: &str) -> anyhow::Result<ObjectPath> {
    if key.is_empty() || key.starts_with('/') || key.contains("..") {
        bail!("invalid firmware object key");
    }
    ObjectPath::parse(key).context("invalid firmware object key")
}

fn safe_segment(value: &str, fallback: &str) -> String {
    let segment: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(160)
        .collect();
    if segment.is_empty() {
        fallback.to_string()
    } else {
        segment
    }
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "storage tests stay adjacent to the object-key helpers they cover"
)]
mod tests {
    use super::FirmwareObjectStore;
    use uuid::Uuid;

    #[tokio::test]
    async fn round_trips_firmware_objects() {
        let store = FirmwareObjectStore::in_memory();
        let key = store.allocate_key("tenant/acme", "../device firmware.bin");

        assert!(!key.contains(".."));
        store.put(&key, vec![1, 2, 3]).await.unwrap();
        assert_eq!(store.get(&key).await.unwrap(), vec![1, 2, 3]);
        store.delete(&key).await.unwrap();
        assert!(store.get(&key).await.is_err());
    }

    #[test]
    fn allocated_object_key_is_tenant_scoped_and_unique() {
        let store = FirmwareObjectStore::in_memory();
        let key = store.allocate_key("tenant/acme", "../device firmware.bin");
        let segments = key.split('/').collect::<Vec<_>>();

        assert_eq!(segments[0..3], ["tenants", "tenant_acme", "firmware"]);
        assert!(Uuid::parse_str(segments[3]).is_ok());
        assert_eq!(segments[4], "___device_firmware_bin");
        assert_eq!(segments.len(), 5);
        assert_ne!(
            key,
            store.allocate_key("tenant/acme", "../device firmware.bin")
        );
    }
}

#[async_trait::async_trait]
impl extrittio_backend_core::firmware::FirmwareObjectStorage for FirmwareObjectStore {
    fn backend(&self) -> &str {
        FirmwareObjectStore::backend(self)
    }
    fn allocate_key(&self, tenant: &extrittio_backend_core::TenantId, filename: &str) -> String {
        FirmwareObjectStore::allocate_key(self, tenant.as_str(), filename)
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, String> {
        FirmwareObjectStore::get(self, key).await.map_err(|error| {
            tracing::error!(%error, "Firmware object download failed");
            error.to_string()
        })
    }
    async fn put(&self, key: &str, data: Vec<u8>) -> Result<(), String> {
        FirmwareObjectStore::put(self, key, data)
            .await
            .map_err(|error| {
                tracing::error!(%error, "Firmware object upload failed");
                error.to_string()
            })
    }
    async fn delete(&self, key: &str) -> Result<(), String> {
        FirmwareObjectStore::delete(self, key)
            .await
            .map_err(|error| {
                tracing::error!(%error, "Failed to clean up unreferenced firmware object");
                error.to_string()
            })
    }
}
