use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::error::AppError;
use crate::services::cert_service;
use crate::state::{AppState, run_db};

#[derive(Debug, Serialize, ToSchema)]
pub struct CaCertificateResponse {
    pub fingerprint: String,
    pub certificate_pem: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceCertificateResponse {
    pub certificate_pem: String,
    pub private_key_pem: String,
    pub ca_pem: String,
    pub fingerprint: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
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

/// Get the CA certificate.
#[utoipa::path(
    get,
    path = "/api/v1/ca/certificate",
    tag = "certificates",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "CA certificate", body = CaCertificateResponse),
        (status = 404, description = "CA certificate not initialized"),
    ),
)]
pub(crate) async fn get_ca_certificate(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CaCertificateResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let ca = cert_service::get_ca_certificate(conn)?
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
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/certificate",
    tag = "certificates",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Device certificate bundle", body = DeviceCertificateResponse),
        (status = 400, description = "Private key already downloaded"),
        (status = 404, description = "Device or certificate not found"),
    ),
)]
pub(crate) async fn get_device_certificate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DeviceCertificateResponse>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let bundle = cert_service::get_device_certificate_bundle(conn, &id)?;

        let private_key_pem = bundle.private_key_pem.ok_or_else(|| {
            AppError::BadRequest(
                "Private key already downloaded. Use /regenerate to issue a new certificate."
                    .into(),
            )
        })?;

        Ok(DeviceCertificateResponse {
            certificate_pem: bundle.device_cert.certificate_pem,
            private_key_pem,
            ca_pem: bundle.ca_cert_pem,
            fingerprint: bundle.device_cert.fingerprint,
            expires_at: bundle.device_cert.expires_at.to_string(),
            created_at: bundle.device_cert.created_at.to_string(),
        })
    })
    .await?;

    Ok(Json(response))
}

/// Regenerate a device certificate.
#[utoipa::path(
    post,
    path = "/api/v1/devices/{id}/certificate/regenerate",
    tag = "certificates",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 201, description = "Certificate regenerated", body = DeviceCertificateResponse),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn regenerate_device_certificate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<DeviceCertificateResponse>), AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let cert = cert_service::regenerate_device_certificate(conn, &id)?;

        let ca = cert_service::get_ca_certificate(conn)?
            .ok_or_else(|| AppError::Internal("CA certificate not initialized".into()))?;

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

/// Get the status of a device certificate (without the private key).
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/certificate/status",
    tag = "certificates",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    responses(
        (status = 200, description = "Certificate status", body = Option<DeviceCertificateStatusResponse>),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn get_device_certificate_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Option<DeviceCertificateStatusResponse>>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        let cert = cert_service::get_device_certificate_status(conn, &id)?;

        Ok(cert.map(|c| DeviceCertificateStatusResponse {
            fingerprint: c.fingerprint,
            expires_at: c.expires_at.to_string(),
            created_at: c.created_at.to_string(),
        }))
    })
    .await?;

    Ok(Json(response))
}
