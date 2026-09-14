use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use chrono::{Duration, Utc};
use rcgen::{
    CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose,
};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};

use async_trait::async_trait;
use extrittio_backend_core::{ApplicationError, TenantId, certificates::*};
use std::sync::Arc;

const ENCRYPTED_KEY_PREFIX: &str = "enc:v1:";
const KEY_ENCRYPTION_SECRET_ENV: &str = "EXTRITTIO_KEY_ENCRYPTION_SECRET";

/// Configured once at composition; never reads process environment during a use case.
#[derive(Clone)]
pub struct CertificateCrypto {
    secret: Option<Arc<str>>,
}
impl CertificateCrypto {
    pub fn new(secret: Option<String>) -> Self {
        Self {
            secret: secret.map(Arc::from),
        }
    }
    /// Generate a self-signed root CA certificate (Ed25519, 10-year validity).
    fn generate_ca_certificate(&self) -> Result<NewCaCertificateRecord, ApplicationError> {
        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ED25519).map_err(|e| {
            ApplicationError::Internal(format!("Failed to generate CA key pair: {e}"))
        })?;

        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| ApplicationError::Internal(format!("Failed to create CA params: {e}")))?;

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

        let ca_cert = params.self_signed(&key_pair).map_err(|e| {
            ApplicationError::Internal(format!("Failed to self-sign CA certificate: {e}"))
        })?;

        Ok(NewCaCertificateRecord {
            private_key_pem: self.protect(&key_pair.serialize_pem())?,
            certificate_pem: ca_cert.pem(),
        })
    }

    /// Generate a tenant-owned device certificate signed by the CA.
    /// CN is set to the device ID.
    fn generate_device_certificate_for_tenant(
        &self,
        tenant_id: &str,
        device_id: &str,
        ca: &CaCertificateRecord,
    ) -> Result<NewDeviceCertificateRecord, ApplicationError> {
        let ca_private_key_pem = self.unprotect(&ca.private_key_pem)?;
        let ca_key_pair = KeyPair::from_pem(&ca_private_key_pem)
            .map_err(|e| ApplicationError::Internal(format!("Failed to parse CA key: {e}")))?;

        let issuer = Issuer::from_ca_cert_pem(&ca.certificate_pem, ca_key_pair).map_err(|e| {
            ApplicationError::Internal(format!("Failed to parse CA certificate: {e}"))
        })?;

        // Generate device key pair
        let device_key_pair = KeyPair::generate_for(&rcgen::PKCS_ED25519).map_err(|e| {
            ApplicationError::Internal(format!("Failed to generate device key pair: {e}"))
        })?;

        let mut params = CertificateParams::new(Vec::<String>::new()).map_err(|e| {
            ApplicationError::Internal(format!("Failed to create device cert params: {e}"))
        })?;

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

        let device_cert = params.signed_by(&device_key_pair, &issuer).map_err(|e| {
            ApplicationError::Internal(format!("Failed to sign device certificate: {e}"))
        })?;

        let cert_pem = device_cert.pem();

        // Compute SHA-256 fingerprint of the DER-encoded certificate
        let cert_der = device_cert.der();
        let fingerprint = compute_fingerprint(cert_der);

        let expires_at = Utc::now() + Duration::days(365);

        let device_private_key_pem = device_key_pair.serialize_pem();

        let _ = tenant_id;
        Ok(NewDeviceCertificateRecord {
            device_id: device_id.to_string(),
            private_key_pem: self.protect(&device_private_key_pem)?,
            certificate_pem: cert_pem,
            fingerprint,
            expires_at,
        })
    }

    /// Generate a server certificate signed by the CA for Zenoh TLS.
    fn generate_server_certificate(
        &self,
        ca: &CaCertificateRecord,
    ) -> Result<(String, String), ApplicationError> {
        let ca_private_key_pem = self.unprotect(&ca.private_key_pem)?;
        let ca_key_pair = KeyPair::from_pem(&ca_private_key_pem)
            .map_err(|e| ApplicationError::Internal(format!("Failed to parse CA key: {e}")))?;

        let issuer = Issuer::from_ca_cert_pem(&ca.certificate_pem, ca_key_pair).map_err(|e| {
            ApplicationError::Internal(format!("Failed to parse CA certificate: {e}"))
        })?;

        let server_key_pair = KeyPair::generate_for(&rcgen::PKCS_ED25519).map_err(|e| {
            ApplicationError::Internal(format!("Failed to generate server key pair: {e}"))
        })?;

        let mut params = CertificateParams::new(vec!["localhost".to_string()]).map_err(|e| {
            ApplicationError::Internal(format!("Failed to create server cert params: {e}"))
        })?;

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

        let server_cert = params.signed_by(&server_key_pair, &issuer).map_err(|e| {
            ApplicationError::Internal(format!("Failed to sign server certificate: {e}"))
        })?;

        Ok((server_cert.pem(), server_key_pair.serialize_pem()))
    }
}
impl KeyProtector for CertificateCrypto {
    fn enabled(&self) -> bool {
        self.secret.is_some()
    }
    fn is_protected(&self, value: &str) -> bool {
        is_protected_private_key(value)
    }
    fn protect(&self, private_key_pem: &str) -> Result<String, ApplicationError> {
        let Some(secret) = self.secret.as_deref() else {
            return Ok(private_key_pem.to_string());
        };

        let mut nonce_bytes = [0_u8; 12];
        SystemRandom::new().fill(&mut nonce_bytes).map_err(|_| {
            ApplicationError::Internal("Failed to generate private-key nonce".into())
        })?;

        protect_private_key_with_secret_and_nonce(private_key_pem, &secret, nonce_bytes)
    }

    fn unprotect(&self, stored_value: &str) -> Result<String, ApplicationError> {
        if !is_protected_private_key(stored_value) {
            return Ok(stored_value.to_string());
        }

        let Some(secret) = self.secret.as_deref() else {
            return Err(ApplicationError::Internal(format!(
                "{KEY_ENCRYPTION_SECRET_ENV} is required to decrypt stored private keys"
            )));
        };

        unprotect_private_key_with_secret(stored_value, &secret)
    }
}
#[async_trait]
impl CertificateIssuer for CertificateCrypto {
    async fn generate_ca(&self) -> Result<NewCaCertificateRecord, ApplicationError> {
        let crypto = self.clone();
        tokio::task::spawn_blocking(move || crypto.generate_ca_certificate())
            .await
            .map_err(|error| {
                ApplicationError::Internal(format!("CA generation task failed: {error}"))
            })?
    }
    async fn generate_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
        ca: CaCertificateRecord,
    ) -> Result<NewDeviceCertificateRecord, ApplicationError> {
        let crypto = self.clone();
        let tenant = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        tokio::task::spawn_blocking(move || {
            crypto.generate_device_certificate_for_tenant(&tenant, &device_id, &ca)
        })
        .await
        .map_err(|error| ApplicationError::Internal(format!("certificate task failed: {error}")))?
    }
    async fn generate_server(
        &self,
        ca: CaCertificateRecord,
    ) -> Result<(String, String), ApplicationError> {
        let crypto = self.clone();
        tokio::task::spawn_blocking(move || crypto.generate_server_certificate(&ca))
            .await
            .map_err(|error| {
                ApplicationError::Internal(format!("server certificate task failed: {error}"))
            })?
    }
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
pub fn fingerprint_from_pem(pem_str: &str) -> Result<String, ApplicationError> {
    let parsed = pem::parse(pem_str)
        .map_err(|e| ApplicationError::Internal(format!("Failed to parse PEM: {e}")))?;
    Ok(compute_fingerprint(parsed.contents()))
}

fn is_protected_private_key(value: &str) -> bool {
    value.starts_with(ENCRYPTED_KEY_PREFIX)
}

fn protect_private_key_with_secret_and_nonce(
    private_key_pem: &str,
    secret: &str,
    nonce_bytes: [u8; 12],
) -> Result<String, ApplicationError> {
    let key_bytes = key_encryption_key(secret);
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| {
        ApplicationError::Internal("Failed to create private-key encryption key".into())
    })?);

    let mut encrypted = private_key_pem.as_bytes().to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce_bytes),
        Aad::empty(),
        &mut encrypted,
    )
    .map_err(|_| ApplicationError::Internal("Failed to encrypt private key".into()))?;

    Ok(format!(
        "{ENCRYPTED_KEY_PREFIX}{}:{}",
        STANDARD_NO_PAD.encode(nonce_bytes),
        STANDARD_NO_PAD.encode(encrypted)
    ))
}

fn unprotect_private_key_with_secret(
    stored_value: &str,
    secret: &str,
) -> Result<String, ApplicationError> {
    if !is_protected_private_key(stored_value) {
        return Ok(stored_value.to_string());
    }

    let encrypted = stored_value.trim_start_matches(ENCRYPTED_KEY_PREFIX);
    let (nonce, ciphertext) = encrypted
        .split_once(':')
        .ok_or_else(|| ApplicationError::Internal("Invalid encrypted private-key format".into()))?;
    let nonce_bytes = STANDARD_NO_PAD
        .decode(nonce)
        .map_err(|_| ApplicationError::Internal("Invalid private-key nonce encoding".into()))?;
    let nonce_bytes: [u8; 12] = nonce_bytes
        .try_into()
        .map_err(|_| ApplicationError::Internal("Invalid private-key nonce length".into()))?;
    let mut ciphertext = STANDARD_NO_PAD.decode(ciphertext).map_err(|_| {
        ApplicationError::Internal("Invalid private-key ciphertext encoding".into())
    })?;

    let key_bytes = key_encryption_key(secret);
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|_| {
        ApplicationError::Internal("Failed to create private-key encryption key".into())
    })?);
    let plaintext = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::empty(),
            &mut ciphertext,
        )
        .map_err(|_| ApplicationError::Internal("Failed to decrypt private key".into()))?;

    String::from_utf8(plaintext.to_vec())
        .map_err(|_| ApplicationError::Internal("Decrypted private key is not UTF-8".into()))
}

fn key_encryption_key(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    const ENCRYPTED_PRIVATE_KEY_V1: &str =
        include_str!("../../tests/fixtures/encrypted-private-key-v1.json");

    #[test]
    fn encrypted_private_key_v1_fixture_remains_readable_and_writable() {
        let fixture: serde_json::Value = serde_json::from_str(ENCRYPTED_PRIVATE_KEY_V1).unwrap();
        let secret = fixture["secret"].as_str().unwrap();
        let plaintext = fixture["plaintext"].as_str().unwrap();
        let stored = fixture["stored"].as_str().unwrap();
        let nonce: [u8; 12] = fixture["nonce_hex"]
            .as_str()
            .unwrap()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        assert_eq!(
            unprotect_private_key_with_secret(stored, secret).unwrap(),
            plaintext
        );
        assert_eq!(
            protect_private_key_with_secret_and_nonce(plaintext, secret, nonce).unwrap(),
            stored
        );
    }

    #[test]
    fn legacy_plaintext_private_key_remains_readable() {
        let plaintext = "-----BEGIN PRIVATE KEY-----\nlegacy\n-----END PRIVATE KEY-----\n";
        assert_eq!(
            unprotect_private_key_with_secret(plaintext, "unused").unwrap(),
            plaintext
        );
    }
}
