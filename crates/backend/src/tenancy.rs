use std::fmt;

pub const DEFAULT_TENANT_ID: &str = "default";

/// Stable tenant identifier carried through request and service boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(value: impl Into<String>) -> Result<Self, TenantIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(TenantIdError::Empty);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Authenticated identity used by device-originated ingestion paths.
///
/// Zenoh topics contain the device ID for protocol compatibility, while the
/// tenant is resolved from the persisted device/certificate identity before
/// any tenant-owned data is read or written. Keeping both values together
/// prevents handlers from silently falling back to the default tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceIdentity {
    tenant_id: TenantId,
    device_id: String,
}

impl DeviceIdentity {
    pub fn new(
        tenant_id: impl Into<String>,
        device_id: impl Into<String>,
    ) -> Result<Self, DeviceIdentityError> {
        let tenant_id = TenantId::new(tenant_id).map_err(DeviceIdentityError::InvalidTenant)?;
        let device_id = device_id.into();
        if !extrittio_common::topics::is_valid_device_id(&device_id) {
            return Err(DeviceIdentityError::InvalidDeviceId);
        }
        Ok(Self {
            tenant_id,
            device_id,
        })
    }

    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    #[must_use]
    pub fn tenant_id_str(&self) -> &str {
        self.tenant_id.as_str()
    }

    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TenantIdError {
    #[error("tenant id must not be empty")]
    Empty,
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceIdentityError {
    #[error("invalid tenant identity: {0}")]
    InvalidTenant(#[source] TenantIdError),
    #[error("device id is invalid")]
    InvalidDeviceId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_identity_rejects_an_empty_tenant() {
        assert!(matches!(
            DeviceIdentity::new("", "device-1"),
            Err(DeviceIdentityError::InvalidTenant(TenantIdError::Empty))
        ));
    }

    #[test]
    fn device_identity_rejects_an_invalid_device_id() {
        assert!(matches!(
            DeviceIdentity::new("tenant-a", "device/one"),
            Err(DeviceIdentityError::InvalidDeviceId)
        ));
    }
}
