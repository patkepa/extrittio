use crate::{ApplicationError, certificates::*};
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// Startup/maintenance façade, composed separately from tenant HTTP application state.
#[derive(Clone)]
pub struct CertificateSystemApplication {
    repository: Arc<dyn CertificateRepository>,
    issuer: Arc<dyn CertificateIssuer>,
    protector: Arc<dyn KeyProtector>,
}
impl CertificateSystemApplication {
    pub fn new(
        repository: Arc<dyn CertificateRepository>,
        issuer: Arc<dyn CertificateIssuer>,
        protector: Arc<dyn KeyProtector>,
    ) -> Self {
        Self {
            repository,
            issuer,
            protector,
        }
    }
    pub async fn initialize_ca(&self) -> Result<bool, ApplicationError> {
        if self.repository.get_ca().await?.is_some() {
            return Ok(false);
        }
        let material = self.issuer.generate_ca().await?;
        self.repository.insert_ca_if_absent(material).await?;
        Ok(true)
    }
    pub async fn ca(&self) -> Result<Option<CaCertificateRecord>, ApplicationError> {
        Ok(self.repository.get_ca().await?)
    }
    pub async fn server_certificate(
        &self,
        ca: CaCertificateRecord,
    ) -> Result<(String, String), ApplicationError> {
        self.issuer.generate_server(ca).await
    }
    /// CN-only ACL is valid because both schemas enforce globally unique device IDs.
    pub async fn active_device_ids(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<String>, ApplicationError> {
        Ok(self.repository.list_active_device_ids(now).await?)
    }
    pub async fn encrypt_stored_private_keys(&self) -> Result<(), ApplicationError> {
        if !self.protector.enabled() {
            return Ok(());
        }

        for record in self.repository.list_stored_private_keys().await? {
            match record {
                StoredPrivateKeyRecord::Ca { id, value }
                    if !self.protector.is_protected(&value) =>
                {
                    let protected = self.protector.protect(&value)?;
                    self.repository
                        .replace_ca_private_key_if_matches(id, value, protected)
                        .await?;
                }
                StoredPrivateKeyRecord::Device {
                    tenant_id,
                    id,
                    value,
                } if !self.protector.is_protected(&value) => {
                    let protected = self.protector.protect(&value)?;
                    self.repository
                        .replace_device_private_key_if_matches(tenant_id, id, value, protected)
                        .await?;
                }
                StoredPrivateKeyRecord::Ca { .. } | StoredPrivateKeyRecord::Device { .. } => {}
            }
        }

        Ok(())
    }
}
