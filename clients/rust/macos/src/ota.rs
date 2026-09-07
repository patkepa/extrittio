use std::sync::Arc;
use std::time::Duration;

use prost::Message;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use extrittio_common::extrittio::ShadowReport;
use extrittio_common::ota::fields as ota_fields;

/// Send a shadow report with the current reported state.
pub async fn send_shadow_report(
    device_id: &str,
    session: &zenoh::Session,
    topic: &str,
    reported_state: &Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    version: i64,
) {
    let state = reported_state.lock().await;
    let state_json = serde_json::to_string(&*state).unwrap_or_else(|_| "{}".to_string());
    tracing::info!("Applied. Reported state: {}", state_json);

    let report = ShadowReport {
        device_id: device_id.to_string(),
        timestamp: extrittio_sdk::time::now_millis(),
        state_json,
        version,
    };

    let payload = report.encode_to_vec();
    if let Err(e) = session.put(topic, payload).await {
        tracing::warn!("Failed to send ShadowReport: {}", e);
    } else {
        tracing::info!("ShadowReport sent (version: {})", version);
    }
}

#[allow(clippy::too_many_arguments)]
async fn report_ota_status(
    reported_state: &Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    device_id: &str,
    session: &zenoh::Session,
    topic: &str,
    version: i64,
    fw_version: &str,
    fw_update_id: Option<i64>,
    deployment_id: i64,
    status: &str,
    error: Option<&str>,
) {
    let ota_obj = extrittio_sdk::ota::build_status_json(
        status,
        fw_version,
        fw_update_id,
        deployment_id,
        error,
    );

    {
        let mut state = reported_state.lock().await;
        state.insert(ota_fields::SHADOW_KEY.to_string(), ota_obj);
    }

    send_shadow_report(device_id, session, topic, reported_state, version).await;
}

/// Handle an OTA update. Downloads firmware, verifies SHA-256, replaces binary,
/// journals the pending installation, then exits for launchd restart.
///
/// Unlike the Linux/RPi clients, this does NOT patch a device ID into the binary.
/// Device identity is stored in the config file and survives OTA naturally.
#[allow(clippy::too_many_lines)]
pub async fn handle_ota(
    ota_payload: serde_json::Value,
    device_id: String,
    session: Arc<zenoh::Session>,
    report_topic: String,
    reported_state: Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    firmware_version: Arc<Mutex<String>>,
    shadow_version: i64,
) {
    let parsed = match extrittio_sdk::ota::OtaPayload::from_json(&ota_payload) {
        Some(p) => p,
        None => {
            tracing::warn!("OTA payload missing required fields");
            return;
        }
    };
    let fw_version = parsed.firmware_version;
    let fw_url = parsed.firmware_url;
    let fw_update_id = parsed.firmware_update_id;
    let deployment_id = parsed.deployment_id;
    let expected_sha256 = parsed.sha256;

    if std::env::current_exe()
        .ok()
        .is_some_and(|exe| extrittio_sdk::native_ota::is_installed_attempt(&exe, deployment_id))
    {
        return;
    }

    // Download
    tracing::info!("OTA: downloading firmware v{}", fw_version);
    report_ota_status(
        &reported_state,
        &device_id,
        &session,
        &report_topic,
        shadow_version,
        &fw_version,
        fw_update_id,
        deployment_id,
        "downloading",
        None,
    )
    .await;

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let bytes = match http_client.get(&fw_url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                let err = format!("HTTP {}", resp.status());
                tracing::warn!("OTA: download failed: {}", err);
                report_ota_status(
                    &reported_state,
                    &device_id,
                    &session,
                    &report_topic,
                    shadow_version,
                    &fw_version,
                    fw_update_id,
                    deployment_id,
                    "failed",
                    Some(&err),
                )
                .await;
                return;
            }
            match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    let err = format!("download read error: {}", e.without_url());
                    tracing::warn!("OTA: {}", err);
                    report_ota_status(
                        &reported_state,
                        &device_id,
                        &session,
                        &report_topic,
                        shadow_version,
                        &fw_version,
                        fw_update_id,
                        deployment_id,
                        "failed",
                        Some(&err),
                    )
                    .await;
                    return;
                }
            }
        }
        Err(e) => {
            let err = format!("download error: {}", e.without_url());
            tracing::warn!("OTA: {}", err);
            report_ota_status(
                &reported_state,
                &device_id,
                &session,
                &report_topic,
                shadow_version,
                &fw_version,
                fw_update_id,
                deployment_id,
                "failed",
                Some(&err),
            )
            .await;
            return;
        }
    };

    tracing::info!("OTA: downloaded {} bytes", bytes.len());

    // Verify SHA-256 on raw downloaded bytes before any filesystem operations
    if let Some(ref expected) = expected_sha256 {
        report_ota_status(
            &reported_state,
            &device_id,
            &session,
            &report_topic,
            shadow_version,
            &fw_version,
            fw_update_id,
            deployment_id,
            "verifying",
            None,
        )
        .await;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = format!("{:x}", hasher.finalize());

        if actual.to_lowercase() != expected.to_lowercase() {
            let err = format!("hash mismatch: expected={expected} got={actual}");
            tracing::warn!("OTA: {}", err);
            report_ota_status(
                &reported_state,
                &device_id,
                &session,
                &report_topic,
                shadow_version,
                &fw_version,
                fw_update_id,
                deployment_id,
                "failed",
                Some(&err),
            )
            .await;
            return;
        }
        tracing::info!("OTA: SHA-256 verified");
    }

    // Install: write to temp file, chmod, atomic rename
    report_ota_status(
        &reported_state,
        &device_id,
        &session,
        &report_topic,
        shadow_version,
        &fw_version,
        fw_update_id,
        deployment_id,
        "installing",
        None,
    )
    .await;

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            let err = format!("cannot resolve current executable path: {e}");
            tracing::warn!("OTA: {}", err);
            report_ota_status(
                &reported_state,
                &device_id,
                &session,
                &report_topic,
                shadow_version,
                &fw_version,
                fw_update_id,
                deployment_id,
                "failed",
                Some(&err),
            )
            .await;
            return;
        }
    };

    let previous_version = firmware_version.lock().await.clone();
    if let Err(error) =
        extrittio_sdk::native_ota::install(&current_exe, &bytes, &ota_payload, &previous_version)
    {
        report_ota_status(
            &reported_state,
            &device_id,
            &session,
            &report_topic,
            shadow_version,
            &fw_version,
            fw_update_id,
            deployment_id,
            "failed",
            Some(&error.to_string()),
        )
        .await;
        return;
    }
    report_ota_status(
        &reported_state,
        &device_id,
        &session,
        &report_topic,
        shadow_version,
        &fw_version,
        fw_update_id,
        deployment_id,
        "rebooting",
        None,
    )
    .await;

    tracing::info!(
        "OTA: binary replaced at {}. Exiting for launchd restart.",
        current_exe.display()
    );

    tokio::time::sleep(Duration::from_secs(2)).await;
    std::process::exit(0);
}
