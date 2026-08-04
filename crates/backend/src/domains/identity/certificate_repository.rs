use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::certificate_types::{
    CaCertificateRecord, CertificateMaterialOutcome, CertificateStatusOutcome,
    NewCaCertificateRecord, NewDeviceCertificateRecord, ReplaceCertificateOutcome,
    StoredPrivateKeyRecord,
};

#[async_trait]
pub trait CertificateRepository: Send + Sync {
    async fn get_ca(&self) -> Result<Option<CaCertificateRecord>, PersistenceError>;

    async fn insert_ca_if_absent(
        &self,
        record: NewCaCertificateRecord,
    ) -> Result<CaCertificateRecord, PersistenceError>;

    async fn list_stored_private_keys(
        &self,
    ) -> Result<Vec<StoredPrivateKeyRecord>, PersistenceError>;

    async fn replace_ca_private_key_if_matches(
        &self,
        certificate_id: i32,
        expected_value: String,
        replacement: String,
    ) -> Result<bool, PersistenceError>;

    async fn replace_device_private_key_if_matches(
        &self,
        tenant_id: String,
        certificate_id: i32,
        expected_value: String,
        replacement: String,
    ) -> Result<bool, PersistenceError>;

    async fn get_download_material(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<CertificateMaterialOutcome, PersistenceError>;

    async fn consume_private_key(
        &self,
        tenant: &TenantId,
        certificate_id: i32,
        expected_value: String,
    ) -> Result<bool, PersistenceError>;

    async fn get_status(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<CertificateStatusOutcome, PersistenceError>;

    async fn replace_device_certificate(
        &self,
        tenant: &TenantId,
        record: NewDeviceCertificateRecord,
    ) -> Result<ReplaceCertificateOutcome, PersistenceError>;

    async fn list_active_device_ids(
        &self,
        active_at: DateTime<Utc>,
    ) -> Result<Vec<String>, PersistenceError>;
}
