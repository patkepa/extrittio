use prost::Message;
use std::sync::Arc;
use tracing::{info, warn};

use crate::persistence::Persistence;
use crate::services::{device_catalog_service, shadow_service};
use crate::state::{DbPool, ZenohMetrics};
use crate::tenancy::DeviceIdentity;

use extrittio_common::extrittio::{ShadowGet, ShadowReport};

async fn resolve_identity(
    persistence: &Persistence,
    message_type: &'static str,
    device_id: &str,
) -> Option<DeviceIdentity> {
    match device_catalog_service::resolve_identity(persistence.devices.as_ref(), device_id).await {
        Ok(Some(identity)) => Some(identity),
        Ok(None) => {
            warn!("Dropping {message_type} from unregistered device: {device_id}");
            None
        }
        Err(error) => {
            warn!("Failed to resolve {message_type} device identity: {error}");
            None
        }
    }
}

/// Decode a `ShadowReport`, atomically merge reported state, and then update
/// legacy OTA status bookkeeping until that write set moves to its own port.
pub async fn handle_shadow_report(
    db_pool: &DbPool,
    persistence: &Persistence,
    topic_device_id: &str,
    payload: &[u8],
) {
    let report = match ShadowReport::decode(payload) {
        Ok(message) => message,
        Err(error) => {
            warn!("Failed to decode ShadowReport: {error}");
            return;
        }
    };
    if !super::validate_topic_device("shadow report", topic_device_id, &report.device_id) {
        return;
    }

    let Some(identity) = resolve_identity(persistence, "shadow report", &report.device_id).await
    else {
        return;
    };
    let reported: serde_json::Value = match serde_json::from_str(&report.state_json) {
        Ok(value) => value,
        Err(error) => {
            warn!("Invalid JSON in ShadowReport: {error}");
            return;
        }
    };
    let Some(patch) = reported.as_object().cloned() else {
        warn!(
            "ShadowReport state_json is not a JSON object for device {}",
            report.device_id
        );
        return;
    };

    let shadow = match shadow_service::update_reported_for_tenant(
        persistence.shadows.as_ref(),
        identity.tenant_id(),
        identity.device_id(),
        &patch,
    )
    .await
    {
        Ok(shadow) => shadow,
        Err(error) => {
            warn!("Failed to update shadow reported state: {error}");
            return;
        }
    };

    if report.version != 0 && report.version != i64::from(shadow.version) {
        warn!(
            "Shadow version mismatch for device {}: device reported version={}, server version={}",
            report.device_id, report.version, shadow.version
        );
    }
    info!(
        "Shadow report from device {}: version={}",
        report.device_id, shadow.version
    );

    let pool = db_pool.clone();
    let tenant_id = identity.tenant_id_str().to_string();
    let device_id = identity.device_id().to_string();
    let reported = shadow.reported;
    match tokio::task::spawn_blocking(move || {
        let mut connection = pool.get().map_err(crate::error::AppError::Pool)?;
        shadow_service::process_ota_from_report(&mut connection, &tenant_id, &device_id, &reported)
    })
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => warn!("Failed to process OTA from shadow report: {error}"),
        Err(error) => warn!("Shadow report OTA task failed: {error}"),
    }
}

/// Decode a `ShadowGet` and publish the currently committed delta if non-empty.
pub async fn handle_shadow_get(
    db_pool: &DbPool,
    persistence: &Persistence,
    session: &Arc<zenoh::Session>,
    topic_device_id: &str,
    payload: &[u8],
    zenoh_metrics: &ZenohMetrics,
) {
    let get_message = match ShadowGet::decode(payload) {
        Ok(message) => message,
        Err(error) => {
            warn!("Failed to decode ShadowGet: {error}");
            return;
        }
    };
    if !super::validate_topic_device("shadow get", topic_device_id, &get_message.device_id) {
        return;
    }

    let Some(identity) = resolve_identity(persistence, "shadow get", &get_message.device_id).await
    else {
        return;
    };
    let shadow = match shadow_service::get_shadow_for_tenant(
        persistence.shadows.as_ref(),
        identity.tenant_id(),
        identity.device_id(),
    )
    .await
    {
        Ok(shadow) => shadow,
        Err(error) => {
            warn!(
                "Shadow not found for device {}: {error}",
                get_message.device_id
            );
            return;
        }
    };

    if shadow
        .delta
        .as_object()
        .is_some_and(serde_json::Map::is_empty)
    {
        info!(
            "Shadow get from device {}: already in sync",
            get_message.device_id
        );
        return;
    }

    shadow_service::publish_delta_if_nonempty(
        session,
        &get_message.device_id,
        &shadow.delta,
        shadow.version,
        zenoh_metrics,
    )
    .await;
    info!(
        "Shadow get from device {}: sent delta",
        get_message.device_id
    );
}
