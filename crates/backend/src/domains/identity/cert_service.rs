use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use chrono::{Duration, NaiveDateTime, Utc};
use rcgen::{
    CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose,
};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};

use diesel::PgConnection;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{CaCertificate, NewCaCertificate, NewDeviceCertificate};
use crate::domains::identity::certificate_repository::CertificateRepository;
use crate::domains::identity::certificate_types::{
    CaCertificateRecord, CertificateMaterialOutcome, CertificateStatusOutcome,
    DeviceCertificateRecord, NewDeviceCertificateRecord, ReplaceCertificateOutcome,
    StoredPrivateKeyRecord,
};
use crate::error::AppError;
use crate::repositories::cert_repo;
use crate::tenancy::DEFAULT_TENANT_ID;

const ENCRYPTED_KEY_PREFIX: &str = "enc:v1:";
const KEY_ENCRYPTION_SECRET_ENV: &str = "EXTRITTIO_KEY_ENCRYPTION_SECRET";

/// Generate a self-signed root CA certificate (Ed25519, 10-year validity).
pub fn generate_ca_certificate() -> Result<NewCaCertificate, AppError> {
    let key_pair = KeyPair::generate_for(&rcgen::PKCS_ED25519)
        .map_err(|e| AppError::Internal(format!("Failed to generate CA key pair: {e}")))?;

    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| AppError::Internal(format!("Failed to create CA params: {e}")))?;

    params
        .distinguished_name
        .push(DnType::CommonName, "Extrittio Root CA");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Extrittio");
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];

    // 10 year validity
    let now = time::OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(3650);

    let ca_cert = params
        .self_signed(&key_pair)
        .map_err(|e| AppError::Internal(format!("Failed to self-sign CA certificate: {e}")))?;

    Ok(NewCaCertificate {
        private_key_pem: protect_private_key(&key_pair.serialize_pem())?,
        certificate_pem: ca_cert.pem(),
    })
}

/// Generate a device certificate signed by the CA (Ed25519, 1-year validity).
/// CN is set to the device ID.
pub fn generate_device_certificate(
    device_id: &str,
    ca: &CaCertificate,
) -> Result<NewDeviceCertificate, AppError> {
    generate_device_certificate_for_tenant(DEFAULT_TENANT_ID, device_id, ca)
}

/// Generate a tenant-owned device certificate signed by the CA.
/// CN is set to the device ID.
pub fn generate_device_certificate_for_tenant(
    tenant_id: &str,
    device_id: &str,
    ca: &CaCertificate,
) -> Result<NewDeviceCertificate, AppError> {
    let ca_private_key_pem = unprotect_private_key(&ca.private_key_pem)?;
    let ca_key_pair = KeyPair::from_pem(&ca_private_key_pem)
        .map_err(|e| AppError::Internal(format!("Failed to parse CA key: {e}")))?;

    let issuer = Issuer::from_ca_cert_pem(&ca.certificate_pem, ca_key_pair)
        .map_err(|e| AppError::Internal(format!("Failed to parse CA certificate: {e}")))?;

    // Generate device key pair
    let device_key_pair = KeyPair::generate_for(&rcgen::PKCS_ED25519)
        .map_err(|e| AppError::Internal(format!("Failed to generate device key pair: {e}")))?;

    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| AppError::Internal(format!("Failed to create device cert params: {e}")))?;

    params
        .distinguished_name
        .push(DnType::CommonName, device_id);
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Extrittio");

    // 1 year validity
    let now = time::OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(365);

    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];

    let device_cert = params
        .signed_by(&device_key_pair, &issuer)
        .map_err(|e| AppError::Internal(format!("Failed to sign device certificate: {e}")))?;

    let cert_pem = device_cert.pem();

    // Compute SHA-256 fingerprint of the DER-encoded certificate
    let cert_der = device_cert.der();
    let fingerprint = compute_fingerprint(cert_der);

    let expires_at: NaiveDateTime = (Utc::now() + Duration::days(365)).naive_utc();

    let device_private_key_pem = device_key_pair.serialize_pem();

    Ok(NewDeviceCertificate {
        tenant_id: tenant_id.to_string(),
        device_id: device_id.to_string(),
        private_key_pem: protect_private_key(&device_private_key_pem)?,
        certificate_pem: cert_pem,
        fingerprint,
        expires_at,
    })
}

/// Generate a server certificate signed by the CA for Zenoh TLS.
pub fn generate_server_certificate(ca: &CaCertificate) -> Result<(String, String), AppError> {
    let ca_private_key_pem = unprotect_private_key(&ca.private_key_pem)?;
    let ca_key_pair = KeyPair::from_pem(&ca_private_key_pem)
        .map_err(|e| AppError::Internal(format!("Failed to parse CA key: {e}")))?;

    let issuer = Issuer::from_ca_cert_pem(&ca.certificate_pem, ca_key_pair)
        .map_err(|e| AppError::Internal(format!("Failed to parse CA certificate: {e}")))?;

    let server_key_pair = KeyPair::generate_for(&rcgen::PKCS_ED25519)
        .map_err(|e| AppError::Internal(format!("Failed to generate server key pair: {e}")))?;

    let mut params = CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|e| AppError::Internal(format!("Failed to create server cert params: {e}")))?;

    params
        .distinguished_name
        .push(DnType::CommonName, "Extrittio Server");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Extrittio");

    let now = time::OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(365);

    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];

    let server_cert = params
        .signed_by(&server_key_pair, &issuer)
        .map_err(|e| AppError::Internal(format!("Failed to sign server certificate: {e}")))?;

    Ok((server_cert.pem(), server_key_pair.serialize_pem()))
}

pub async fn encrypt_stored_private_keys(
    repository: &dyn CertificateRepository,
) -> Result<(), AppError> {
    if key_encryption_secret().is_none() {
        return Ok(());
    }

    for record in repository.list_stored_private_keys().await? {
        match record {
            StoredPrivateKeyRecord::Ca { id, value } if !is_protected_private_key(&value) => {
                let protected = protect_private_key(&value)?;
                repository
                    .replace_ca_private_key_if_matches(id, value, protected)
                    .await?;
            }
            StoredPrivateKeyRecord::Device {
                tenant_id,
                id,
                value,
            } if !is_protected_private_key(&value) => {
                let protected = protect_private_key(&value)?;
                repository
                    .replace_device_private_key_if_matches(tenant_id, id, value, protected)
                    .await?;
            }
            StoredPrivateKeyRecord::Ca { .. } | StoredPrivateKeyRecord::Device { .. } => {}
        }
    }

    Ok(())
}

/// Compute SHA-256 fingerprint formatted as colon-separated hex.
pub fn compute_fingerprint(der: &[u8]) -> String {
    let hash = Sha256::digest(der);
    hash.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Compute fingerprint from a PEM certificate string.
pub fn fingerprint_from_pem(pem_str: &str) -> Result<String, AppError> {
    let parsed =
        pem::parse(pem_str).map_err(|e| AppError::Internal(format!("Failed to parse PEM: {e}")))?;
    Ok(compute_fingerprint(parsed.contents()))
}

/// Bundle returned from certificate retrieval.
pub struct CertBundle {
    pub device_cert: DeviceCertificateRecord,
    pub ca_cert_pem: String,
    pub private_key_pem: Option<String>,
}

/// Get the device certificate bundle. One-time private key download.
pub async fn get_device_certificate_bundle(
    ctx: &RequestContext,
    repository: &dyn CertificateRepository,
    device_id: &str,
) -> Result<CertBundle, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;
    let (cert, ca) = match repository
        .get_download_material(ctx.tenant_id(), device_id)
        .await?
    {
        CertificateMaterialOutcome::Found { device, ca } => (device, ca),
        CertificateMaterialOutcome::DeviceNotFound => {
            return Err(AppError::NotFound(format!(
                "Device '{device_id}' not found"
            )));
        }
        CertificateMaterialOutcome::CertificateNotFound => {
            return Err(AppError::NotFound(format!(
                "No certificate found for device '{device_id}'"
            )));
        }
        CertificateMaterialOutcome::CaNotFound => {
            return Err(AppError::Internal("CA certificate not found".into()));
        }
    };

    let private_key = if cert.private_key_pem.is_empty() {
        None
    } else {
        let stored_value = cert.private_key_pem.clone();
        let private_key = unprotect_private_key(&stored_value)?;
        repository
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
    ctx: &RequestContext,
    repository: &dyn CertificateRepository,
    device_id: &str,
) -> Result<Option<DeviceCertificateRecord>, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;
    match repository.get_status(ctx.tenant_id(), device_id).await? {
        CertificateStatusOutcome::Found(certificate) => Ok(certificate),
        CertificateStatusOutcome::DeviceNotFound => Err(AppError::NotFound(format!(
            "Device '{device_id}' not found"
        ))),
    }
}

/// Get the CA certificate, if one has been initialized.
pub fn get_ca_certificate(conn: &mut PgConnection) -> Result<Option<CaCertificate>, AppError> {
    Ok(cert_repo::get_ca_certificate(conn)?)
}

/// Get the CA certificate for an authenticated request.
pub async fn get_ca_certificate_for_request(
    ctx: &RequestContext,
    repository: &dyn CertificateRepository,
) -> Result<Option<CaCertificateRecord>, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;
    Ok(repository.get_ca().await?)
}

/// Delete old certificates and generate a new one.
pub async fn regenerate_device_certificate_bundle(
    ctx: &RequestContext,
    repository: &dyn CertificateRepository,
    device_id: &str,
) -> Result<CertBundle, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;
    let ca = repository
        .get_ca()
        .await?
        .ok_or_else(|| AppError::Internal("CA certificate not found".into()))?;
    let legacy_ca = CaCertificate {
        id: ca.id,
        private_key_pem: ca.private_key_pem.clone(),
        certificate_pem: ca.certificate_pem.clone(),
        created_at: ca.created_at.naive_utc(),
    };
    let new_cert =
        generate_device_certificate_for_tenant(ctx.tenant_id_str(), device_id, &legacy_ca)?;
    let private_key_pem = unprotect_private_key(&new_cert.private_key_pem)?;
    let outcome = repository
        .replace_device_certificate(
            ctx.tenant_id(),
            NewDeviceCertificateRecord {
                device_id: new_cert.device_id,
                private_key_pem: new_cert.private_key_pem,
                certificate_pem: new_cert.certificate_pem,
                fingerprint: new_cert.fingerprint,
                expires_at: new_cert.expires_at.and_utc(),
            },
        )
        .await?;
    let cert = match outcome {
        ReplaceCertificateOutcome::Replaced(certificate) => certificate,
        ReplaceCertificateOutcome::DeviceNotFound => {
            return Err(AppError::NotFound(format!(
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

fn key_encryption_secret() -> Option<String> {
    std::env::var(KEY_ENCRYPTION_SECRET_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_protected_private_key(value: &str) -> bool {
    value.starts_with(ENCRYPTED_KEY_PREFIX)
}

fn protect_private_key(private_key_pem: &str) -> Result<String, AppError> {
    let Some(secret) = key_encryption_secret() else {
        return Ok(private_key_pem.to_string());
    };

    let mut nonce_bytes = [0_u8; 12];
    SystemRandom::new()
        .fill(&mut nonce_bytes)
        .map_err(|_| AppError::Internal("Failed to generate private-key nonce".into()))?;

    let key_bytes = key_encryption_key(&secret);
    let key =
        LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| {
            AppError::Internal("Failed to create private-key encryption key".into())
        })?);

    let mut encrypted = private_key_pem.as_bytes().to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce_bytes),
        Aad::empty(),
        &mut encrypted,
    )
    .map_err(|_| AppError::Internal("Failed to encrypt private key".into()))?;

    Ok(format!(
        "{ENCRYPTED_KEY_PREFIX}{}:{}",
        STANDARD_NO_PAD.encode(nonce_bytes),
        STANDARD_NO_PAD.encode(encrypted)
    ))
}

fn unprotect_private_key(stored_value: &str) -> Result<String, AppError> {
    if !is_protected_private_key(stored_value) {
        return Ok(stored_value.to_string());
    }

    let Some(secret) = key_encryption_secret() else {
        return Err(AppError::Internal(format!(
            "{KEY_ENCRYPTION_SECRET_ENV} is required to decrypt stored private keys"
        )));
    };

    let encrypted = stored_value.trim_start_matches(ENCRYPTED_KEY_PREFIX);
    let (nonce, ciphertext) = encrypted
        .split_once(':')
        .ok_or_else(|| AppError::Internal("Invalid encrypted private-key format".into()))?;
    let nonce_bytes = STANDARD_NO_PAD
        .decode(nonce)
        .map_err(|_| AppError::Internal("Invalid private-key nonce encoding".into()))?;
    let nonce_bytes: [u8; 12] = nonce_bytes
        .try_into()
        .map_err(|_| AppError::Internal("Invalid private-key nonce length".into()))?;
    let mut ciphertext = STANDARD_NO_PAD
        .decode(ciphertext)
        .map_err(|_| AppError::Internal("Invalid private-key ciphertext encoding".into()))?;

    let key_bytes = key_encryption_key(&secret);
    let key =
        LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| {
            AppError::Internal("Failed to create private-key encryption key".into())
        })?);
    let plaintext = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::empty(),
            &mut ciphertext,
        )
        .map_err(|_| AppError::Internal("Failed to decrypt private key".into()))?;

    String::from_utf8(plaintext.to_vec())
        .map_err(|_| AppError::Internal("Decrypted private key is not UTF-8".into()))
}

fn key_encryption_key(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}
