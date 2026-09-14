use super::require_permission;
use crate::certificates::*;
use crate::{ApplicationError, Permission, TenantContext};
use std::sync::Arc;

/// Bundle returned from certificate retrieval.
pub struct CertBundle {
    pub device_cert: DeviceCertificateRecord,
    pub ca_cert_pem: String,
    pub private_key_pem: Option<String>,
}

#[derive(Clone)]
pub struct CertificateApplication {
    repository: Arc<dyn CertificateRepository>,
    issuer: Arc<dyn CertificateIssuer>,
    protector: Arc<dyn KeyProtector>,
}
impl CertificateApplication {
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
    /// Prepare material for the provisioning aggregate, which owns the atomic insert.
    pub async fn prepare_device(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<NewDeviceCertificateRecord>, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        match self.repository.get_ca().await? {
            Some(ca) => Ok(Some(
                self.issuer
                    .generate_device(ctx.tenant_id(), device_id, ca)
                    .await?,
            )),
            None => Ok(None),
        }
    }
    /// Get the device certificate bundle. One-time private key download.
    pub async fn get_device_certificate_bundle(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<CertBundle, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        let (cert, ca) = match self
            .repository
            .get_download_material(ctx.tenant_id(), device_id)
            .await?
        {
            CertificateMaterialOutcome::Found { device, ca } => (device, ca),
            CertificateMaterialOutcome::DeviceNotFound => {
                return Err(ApplicationError::NotFound(format!(
                    "Device '{device_id}' not found"
                )));
            }
            CertificateMaterialOutcome::CertificateNotFound => {
                return Err(ApplicationError::NotFound(format!(
                    "No certificate found for device '{device_id}'"
                )));
            }
            CertificateMaterialOutcome::CaNotFound => {
                return Err(ApplicationError::Internal(
                    "CA certificate not found".into(),
                ));
            }
        };

        let private_key = if cert.private_key_pem.is_empty() {
            None
        } else {
            let stored_value = cert.private_key_pem.clone();
            let private_key = self.protector.unprotect(&stored_value)?;
            self.repository
                .consume_private_key(ctx.tenant_id(), cert.id, stored_value)
                .await?
                .then_some(private_key)
        };

        Ok(CertBundle {
            device_cert: cert,
            ca_cert_pem: ca.certificate_pem,
            private_key_pem: private_key,
        })
    }

    /// Get certificate status (metadata only, no private key).
    pub async fn get_device_certificate_status(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<DeviceCertificateRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        match self
            .repository
            .get_status(ctx.tenant_id(), device_id)
            .await?
        {
            CertificateStatusOutcome::Found(certificate) => Ok(certificate),
            CertificateStatusOutcome::DeviceNotFound => Err(ApplicationError::NotFound(format!(
                "Device '{device_id}' not found"
            ))),
        }
    }

    /// Get the CA certificate for an authenticated request.
    pub async fn get_ca_certificate_for_request(
        &self,
        ctx: &TenantContext,
    ) -> Result<Option<CaCertificateRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        Ok(self.repository.get_ca().await?)
    }

    /// Delete old certificates and generate a new one.
    pub async fn regenerate_device_certificate_bundle(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<CertBundle, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        let ca = self
            .repository
            .get_ca()
            .await?
            .ok_or_else(|| ApplicationError::Internal("CA certificate not found".into()))?;
        let new_cert = self
            .issuer
            .generate_device(ctx.tenant_id(), device_id, ca.clone())
            .await?;
        let private_key_pem = self.protector.unprotect(&new_cert.private_key_pem)?;
        let outcome = self
            .repository
            .replace_device_certificate(
                ctx.tenant_id(),
                NewDeviceCertificateRecord {
                    device_id: new_cert.device_id,
                    private_key_pem: new_cert.private_key_pem,
                    certificate_pem: new_cert.certificate_pem,
                    fingerprint: new_cert.fingerprint,
                    expires_at: new_cert.expires_at,
                },
            )
            .await?;
        let cert = match outcome {
            ReplaceCertificateOutcome::Replaced(certificate) => certificate,
            ReplaceCertificateOutcome::DeviceNotFound => {
                return Err(ApplicationError::NotFound(format!(
                    "Device '{device_id}' not found"
                )));
            }
        };

        Ok(CertBundle {
            device_cert: cert,
            ca_cert_pem: ca.certificate_pem,
            private_key_pem: Some(private_key_pem),
        })
    }
}
