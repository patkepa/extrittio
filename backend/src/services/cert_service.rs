use chrono::{Duration, NaiveDateTime, Utc};
use rcgen::{
    CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose,
};
use sha2::{Digest, Sha256};

use diesel::SqliteConnection;

use crate::db::models::{CaCertificate, DeviceCertificate, NewCaCertificate, NewDeviceCertificate};
use crate::error::AppError;
use crate::repositories::{cert_repo, device_repo};

/// Generate a self-signed root CA certificate (ECDSA P-256, 10-year validity).
pub fn generate_ca_certificate() -> Result<NewCaCertificate, AppError> {
    let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
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
        private_key_pem: key_pair.serialize_pem(),
        certificate_pem: ca_cert.pem(),
    })
}

/// Generate a device certificate signed by the CA (ECDSA P-256, 1-year validity).
/// CN is set to the device ID.
pub fn generate_device_certificate(
    device_id: &str,
    ca: &CaCertificate,
) -> Result<NewDeviceCertificate, AppError> {
    let ca_key_pair = KeyPair::from_pem(&ca.private_key_pem)
        .map_err(|e| AppError::Internal(format!("Failed to parse CA key: {e}")))?;

    let issuer = Issuer::from_ca_cert_pem(&ca.certificate_pem, ca_key_pair)
        .map_err(|e| AppError::Internal(format!("Failed to parse CA certificate: {e}")))?;

    // Generate device key pair
    let device_key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
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

    Ok(NewDeviceCertificate {
        device_id: device_id.to_string(),
        private_key_pem: device_key_pair.serialize_pem(),
        certificate_pem: cert_pem,
        fingerprint,
        expires_at,
    })
}

/// Generate a server certificate signed by the CA for Zenoh TLS.
pub fn generate_server_certificate(ca: &CaCertificate) -> Result<(String, String), AppError> {
    let ca_key_pair = KeyPair::from_pem(&ca.private_key_pem)
        .map_err(|e| AppError::Internal(format!("Failed to parse CA key: {e}")))?;

    let issuer = Issuer::from_ca_cert_pem(&ca.certificate_pem, ca_key_pair)
        .map_err(|e| AppError::Internal(format!("Failed to parse CA certificate: {e}")))?;

    let server_key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
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
    pub device_cert: DeviceCertificate,
    pub ca_cert_pem: String,
    pub private_key_pem: Option<String>,
}

/// Get the device certificate bundle. One-time private key download.
pub fn get_device_certificate_bundle(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<CertBundle, AppError> {
    device_repo::find_device(conn, device_id)?;

    let cert = cert_repo::get_device_certificate(conn, device_id)?
        .ok_or_else(|| AppError::NotFound(format!("No certificate found for device '{device_id}'")))?;

    let ca = cert_repo::get_ca_certificate(conn)?
        .ok_or_else(|| AppError::Internal("CA certificate not found".into()))?;

    let private_key = if !cert.private_key_pem.is_empty() {
        cert_repo::clear_device_private_key(conn, cert.id)?;
        Some(cert.private_key_pem.clone())
    } else {
        None
    };

    Ok(CertBundle {
        device_cert: cert,
        ca_cert_pem: ca.certificate_pem,
        private_key_pem: private_key,
    })
}

/// Get certificate status (metadata only, no private key).
pub fn get_device_certificate_status(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<Option<DeviceCertificate>, AppError> {
    device_repo::find_device(conn, device_id)?;
    Ok(cert_repo::get_device_certificate(conn, device_id)?)
}

/// Get the CA certificate, if one has been initialized.
pub fn get_ca_certificate(
    conn: &mut SqliteConnection,
) -> Result<Option<CaCertificate>, AppError> {
    Ok(cert_repo::get_ca_certificate(conn)?)
}

/// Delete old certificates and generate a new one.
pub fn regenerate_device_certificate(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<DeviceCertificate, AppError> {
    device_repo::find_device(conn, device_id)?;

    let ca = cert_repo::get_ca_certificate(conn)?
        .ok_or_else(|| AppError::Internal("CA certificate not found".into()))?;

    cert_repo::delete_device_certificates(conn, device_id)?;
    let new_cert = generate_device_certificate(device_id, &ca)?;
    Ok(cert_repo::insert_device_certificate(conn, &new_cert)?)
}
