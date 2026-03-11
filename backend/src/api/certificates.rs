use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Serialize;
use std::sync::Arc;

use crate::error::AppError;
use crate::repositories::{cert_repo, device_repo};
use crate::services::cert_service;
use crate::state::{AppState, run_db};

#[derive(Debug, Serialize)]
pub struct CaCertificateResponse {
    pub fingerprint: String,
    pub certificate_pem: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceCertificateResponse {
    pub certificate_pem: String,
    pub private_key_pem: String,
    pub ca_pem: String,
    pub fingerprint: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceCertificateStatusResponse {
    pub fingerprint: String,
    pub expires_at: String,
    pub created_at: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/ca/certificate", get(get_ca_certificate))
        .route(
            "/api/v1/devices/{id}/certificate",
            get(get_device_certificate),
        )
        .route(
            "/api/v1/devices/{id}/certificate/regenerate",
            axum::routing::post(regenerate_device_certificate),
        )
        .route(
            "/api/v1/devices/{id}/certificate/status",
            get(get_device_certificate_status),
        )
}

async fn get_ca_certificate(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CaCertificateResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let ca = cert_repo::get_ca_certificate(conn)?
            .ok_or_else(|| AppError::NotFound("CA certificate not initialized".into()))?;

        let fingerprint = cert_service::fingerprint_from_pem(&ca.certificate_pem)?;

        Ok(CaCertificateResponse {
            fingerprint,
            certificate_pem: ca.certificate_pem,
            created_at: ca.created_at.to_string(),
        })
    })
    .await?;

    Ok(Json(response))
}

/// Download the device certificate bundle (cert + private key + CA cert).
///
/// The private key is only returned once — after download the key is cleared
/// from the database.  Subsequent calls will return 410 Gone if the key has
/// already been retrieved.  Use the `/regenerate` endpoint to issue a new
/// certificate if the key was lost.
async fn get_device_certificate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DeviceCertificateResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        device_repo::find_device(conn, &id)?;

        let cert = cert_repo::get_device_certificate(conn, &id)?
            .ok_or_else(|| AppError::NotFound(format!("No certificate for device '{id}'")))?;

        if cert.private_key_pem.is_empty() {
            return Err(AppError::BadRequest(
                "Private key already downloaded. Use /regenerate to issue a new certificate."
                    .into(),
            ));
        }

        let ca = cert_repo::get_ca_certificate(conn)?
            .ok_or_else(|| AppError::Internal("CA certificate not initialized".into()))?;

        // Clear the private key from the database after retrieval
        cert_repo::clear_device_private_key(conn, cert.id)?;

        Ok(DeviceCertificateResponse {
            certificate_pem: cert.certificate_pem,
            private_key_pem: cert.private_key_pem,
            ca_pem: ca.certificate_pem,
            fingerprint: cert.fingerprint,
            expires_at: cert.expires_at.to_string(),
            created_at: cert.created_at.to_string(),
        })
    })
    .await?;

    Ok(Json(response))
}

async fn regenerate_device_certificate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<DeviceCertificateResponse>), AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        // Verify device exists
        device_repo::find_device(conn, &id)?;

        let ca = cert_repo::get_ca_certificate(conn)?
            .ok_or_else(|| AppError::Internal("CA certificate not initialized".into()))?;

        // Delete existing certificates for this device
        cert_repo::delete_device_certificates(conn, &id)?;

        // Generate new certificate
        let new_cert = cert_service::generate_device_certificate(&id, &ca)?;
        let cert = cert_repo::insert_device_certificate(conn, &new_cert)?;

        Ok(DeviceCertificateResponse {
            certificate_pem: cert.certificate_pem,
            private_key_pem: cert.private_key_pem,
            ca_pem: ca.certificate_pem,
            fingerprint: cert.fingerprint,
            expires_at: cert.expires_at.to_string(),
            created_at: cert.created_at.to_string(),
        })
    })
    .await?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn get_device_certificate_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Option<DeviceCertificateStatusResponse>>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        device_repo::find_device(conn, &id)?;

        let cert = cert_repo::get_device_certificate(conn, &id)?;

        Ok(cert.map(|c| DeviceCertificateStatusResponse {
            fingerprint: c.fingerprint,
            expires_at: c.expires_at.to_string(),
            created_at: c.created_at.to_string(),
        }))
    })
    .await?;

    Ok(Json(response))
}
