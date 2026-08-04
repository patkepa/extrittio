use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaCertificateRecord {
    pub id: i32,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCaCertificateRecord {
    pub private_key_pem: String,
    pub certificate_pem: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCertificateRecord {
    pub id: i32,
    pub device_id: String,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub fingerprint: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDeviceCertificateRecord {
    pub device_id: String,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub fingerprint: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
