//! Certificate persistence and host crypto boundaries.
//! Device operations require a tenant. CA initialization, key-envelope maintenance,
//! and active-certificate ACL enumeration are explicitly system-scoped.
use crate::{ApplicationError, PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Clone, PartialEq, Eq)]
pub struct CaCertificateRecord {
    pub id: i32,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NewCaCertificateRecord {
    pub private_key_pem: String,
    pub certificate_pem: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceCertificateRecord {
    pub id: i32,
    pub device_id: String,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub fingerprint: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NewDeviceCertificateRecord {
    pub device_id: String,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub fingerprint: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum StoredPrivateKeyRecord {
    Ca {
        id: i32,
        value: String,
    },
    Device {
        tenant_id: String,
        id: i32,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateMaterialOutcome {
    Found {
        device: DeviceCertificateRecord,
        ca: CaCertificateRecord,
    },
    DeviceNotFound,
    CertificateNotFound,
    CaNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateStatusOutcome {
    Found(Option<DeviceCertificateRecord>),
    DeviceNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceCertificateOutcome {
    Replaced(DeviceCertificateRecord),
    DeviceNotFound,
}

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

/// Concrete issuance runs in the host, including blocking-task isolation.
#[async_trait]
pub trait CertificateIssuer: Send + Sync {
    async fn generate_ca(&self) -> Result<NewCaCertificateRecord, ApplicationError>;
    async fn generate_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
        ca: CaCertificateRecord,
    ) -> Result<NewDeviceCertificateRecord, ApplicationError>;
    async fn generate_server(
        &self,
        ca: CaCertificateRecord,
    ) -> Result<(String, String), ApplicationError>;
}

/// The host supplies configuration and the existing encrypted-key envelope.
pub trait KeyProtector: Send + Sync {
    fn enabled(&self) -> bool;
    fn is_protected(&self, value: &str) -> bool;
    fn protect(&self, value: &str) -> Result<String, ApplicationError>;
    fn unprotect(&self, value: &str) -> Result<String, ApplicationError>;
}

impl std::fmt::Debug for CaCertificateRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaCertificateRecord")
            .field("private_key_pem", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for NewCaCertificateRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewCaCertificateRecord")
            .field("private_key_pem", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DeviceCertificateRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCertificateRecord")
            .field("private_key_pem", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for NewDeviceCertificateRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewDeviceCertificateRecord")
            .field("private_key_pem", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for StoredPrivateKeyRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ca { id, .. } => f
                .debug_struct("Ca")
                .field("id", id)
                .field("value", &"[REDACTED]")
                .finish(),
            Self::Device { tenant_id, id, .. } => f
                .debug_struct("Device")
                .field("tenant_id", tenant_id)
                .field("id", id)
                .field("value", &"[REDACTED]")
                .finish(),
        }
    }
}
