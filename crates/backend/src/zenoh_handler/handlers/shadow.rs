use prost::Message;
use std::sync::Arc;
use tracing::{info, warn};

use crate::services::shadow_service;
use crate::state::ZenohMetrics;
use crate::tenancy::DeviceIdentity;
use extrittio_backend_core::application::DeviceReportApplication;
use extrittio_backend_core::{DeviceIngressApplication, DeviceShadowApplication};

use extrittio_common::extrittio::{ShadowGet, ShadowReport};

async fn resolve_identity(
    application: &DeviceIngressApplication,
    message_type: &'static str,
    device_id: &str,
) -> Option<DeviceIdentity> {
    match application.resolve_identity(device_id).await {
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
/// process OTA bookkeeping through the core report application.
pub(crate) async fn handle_shadow_report(
    identity_application: &DeviceIngressApplication,
    application: &DeviceReportApplication,
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

    let Some(identity) =
        resolve_identity(identity_application, "shadow report", &report.device_id).await
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

    let outcome = match application.report(&identity, patch).await {
        Ok(shadow) => shadow,
        Err(error) => {
            warn!("Failed to update shadow reported state: {error}");
            return;
        }
    };

    let shadow = outcome.shadow;

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

    if let Some(error) = outcome.firmware_error {
        warn!("Failed to process OTA from shadow report: {error}");
    }
}

/// Decode a `ShadowGet` and publish the currently committed delta if non-empty.
pub(crate) async fn handle_shadow_get(
    identity_application: &DeviceIngressApplication,
    application: &DeviceShadowApplication,
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

    let Some(identity) =
        resolve_identity(identity_application, "shadow get", &get_message.device_id).await
    else {
        return;
    };
    let shadow = match application
        .get(identity.tenant_id(), identity.device_id())
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
