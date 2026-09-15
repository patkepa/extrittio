use crate::{TenantId, TenantIdError};
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
        if !valid_device_id(&device_id) {
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
pub enum DeviceIdentityError {
    #[error("invalid tenant identity: {0}")]
    InvalidTenant(#[source] TenantIdError),
    #[error("device id is invalid")]
    InvalidDeviceId,
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "tests stay next to the public identity contract they cover"
)]
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

// The domain device ID contract matches the established transport/path alphabet.
pub(crate) fn valid_device_id(device_id: &str) -> bool {
    (1..=128).contains(&device_id.len()) && device_id.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b':'))
}
