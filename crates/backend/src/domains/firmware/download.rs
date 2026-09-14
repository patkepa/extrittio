use crate::{error::AppError, tenancy::TenantId};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

const AUDIENCE: &str = "extrittio:firmware-download";

#[derive(Serialize, Deserialize)]
pub struct DownloadGrant {
    pub tenant: String,
    pub firmware_id: i32,
    aud: String,
    exp: u64,
}

pub fn issue(tenant: &TenantId, firmware_id: i32, secret: &str) -> Result<String, AppError> {
    let grant = extrittio_backend_core::firmware::FirmwareDownloadGrant::new(
        tenant.clone(),
        firmware_id,
        &crate::auth::SystemClock,
    )?;
    extrittio_backend_core::firmware::FirmwareDownloadSigner::sign(
        &JwtFirmwareDownloadSigner(secret),
        &grant,
    )
    .map_err(Into::into)
}

pub struct JwtFirmwareDownloadSigner<'a>(pub &'a str);
impl extrittio_backend_core::firmware::FirmwareDownloadSigner for JwtFirmwareDownloadSigner<'_> {
    fn sign(
        &self,
        grant: &extrittio_backend_core::firmware::FirmwareDownloadGrant,
    ) -> Result<String, extrittio_backend_core::ApplicationError> {
        encode(
            &Header::default(),
            &DownloadGrant {
                tenant: grant.tenant.as_str().into(),
                firmware_id: grant.firmware_id,
                aud: grant.audience.into(),
                exp: grant.expires_at,
            },
            &EncodingKey::from_secret(self.0.as_bytes()),
        )
        .map_err(|_| {
            extrittio_backend_core::ApplicationError::Internal(
                "Failed to authorize firmware download".into(),
            )
        })
    }
}

pub fn verify(token: &str, secret: &str) -> Result<DownloadGrant, AppError> {
    let mut validation = Validation::default();
    validation.set_audience(&[AUDIENCE]);
    validation.leeway = 0;
    let grant = decode::<DownloadGrant>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Unauthorized)?
    .claims;
    if grant.firmware_id <= 0 || TenantId::new(grant.tenant.clone()).is_err() {
        return Err(AppError::Unauthorized);
    }
    Ok(grant)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_grants_are_scoped_and_reject_wrong_keys_and_expiry() {
        let tenant = TenantId::new("tenant-a").unwrap();
        let token = issue(&tenant, 42, "test-secret").unwrap();
        let grant = verify(&token, "test-secret").unwrap();
        assert_eq!(grant.tenant, "tenant-a");
        assert_eq!(grant.firmware_id, 42);
        assert!(verify(&token, "wrong-secret").is_err());
        let expired = encode(
            &Header::default(),
            &DownloadGrant { exp: 1, ..grant },
            &EncodingKey::from_secret(b"test-secret"),
        )
        .unwrap();
        assert!(verify(&expired, "test-secret").is_err());
        let other = encode(
            &Header::default(),
            &DownloadGrant {
                tenant: "tenant-a".into(),
                firmware_id: 42,
                aud: "session".into(),
                exp: u64::MAX / 2,
            },
            &EncodingKey::from_secret(b"test-secret"),
        )
        .unwrap();
        assert!(verify(&other, "test-secret").is_err());
    }
}
