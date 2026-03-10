use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use extrittio_proto::extrittio::{
    DeviceHeartbeat, DeviceTelemetry, ShadowDelta, ShadowGet, ShadowReport,
};
use prost::Message;
use rand::Rng;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tracing::info;

// ---------------------------------------------------------------------------
// Embedded device ID
// ---------------------------------------------------------------------------
//
// A fixed slot in the compiled binary: a 23-byte marker followed by 64 bytes
// for the device ID (null-padded) plus 1 trailing null = 88 bytes.
//
// On first provisioning the user passes `--device-id`.  During OTA the handler
// patches this slot in the *downloaded* binary so the new firmware inherits
// the device identity automatically — no CLI argument required after the
// initial run.

const EMBED_MARKER_LEN: usize = 23; // b"<<EXTRITTIO_DEVICE_ID>>"
const EMBED_ID_CAPACITY: usize = 64;

#[used]
#[unsafe(no_mangle)]
pub static DEVICE_ID_EMBED: [u8; 88] = {
    let marker = b"<<EXTRITTIO_DEVICE_ID>>";
    let mut buf = [0u8; 88];
    let mut i = 0;
    while i < 23 {
        buf[i] = marker[i];
        i += 1;
    }
    buf
};

/// Read the device ID from the embedded slot (returns `None` if still blank).
fn read_embedded_device_id() -> Option<String> {
    let id_bytes = &DEVICE_ID_EMBED[EMBED_MARKER_LEN..EMBED_MARKER_LEN + EMBED_ID_CAPACITY];
    let end = id_bytes.iter().position(|&b| b == 0).unwrap_or(id_bytes.len());
    if end == 0 {
        return None;
    }
    String::from_utf8(id_bytes[..end].to_vec()).ok()
}

/// Find the embedded-ID marker in a binary blob and patch the device ID in.
/// The marker is assembled from two halves at runtime so the search literal
/// does not create a second match inside the binary's `.rodata` section.
fn patch_device_id_in_binary(binary: &mut [u8], device_id: &str) -> bool {
    // Search for marker + trailing null bytes to match the actual slot
    // (not a string literal copy that may also exist in the binary).
    let mut needle = Vec::with_capacity(EMBED_MARKER_LEN + 8);
    needle.extend_from_slice(b"<<EXTRITTIO_");
    needle.extend_from_slice(b"DEVICE_ID>>");
    needle.extend_from_slice(&[0u8; 8]);

    if let Some(pos) = binary
        .windows(needle.len())
        .position(|w| w == needle.as_slice())
    {
        let id_start = pos + EMBED_MARKER_LEN;
        let id_bytes = device_id.as_bytes();
        let copy_len = id_bytes.len().min(EMBED_ID_CAPACITY);
        binary[id_start..id_start + copy_len].copy_from_slice(&id_bytes[..copy_len]);
        // Zero-fill the rest of the slot
        for b in &mut binary[id_start + copy_len..id_start + EMBED_ID_CAPACITY] {
            *b = 0;
        }
        true
    } else {
        false
    }
}

#[derive(Parser)]
#[command(name = "extrittio-client", about = "Simulated IoT device client")]
struct Args {
    /// Device ID (reads from embedded firmware ID if omitted)
    #[arg(long)]
    device_id: Option<String>,

    /// Seconds between telemetry messages
    #[arg(long, default_value_t = 5)]
    interval: u64,

    /// Seconds between heartbeat messages
    #[arg(long, default_value_t = 30)]
    heartbeat_interval: u64,
}

struct SensorState {
    temperature: f32,
    humidity: f32,
    battery: f32,
}

impl SensorState {
    fn new() -> Self {
        Self {
            temperature: 22.0,
            humidity: 45.0,
            battery: 100.0,
        }
    }

    fn step(&mut self) {
        let mut rng = rand::rng();

        // Random walk for temperature (±0.5°C per step, clamped to 15-30)
        self.temperature += rng.random_range(-0.5..=0.5);
        self.temperature = self.temperature.clamp(15.0, 30.0);

        // Random walk for humidity (±1% per step, clamped to 20-80)
        self.humidity += rng.random_range(-1.0..=1.0);
        self.humidity = self.humidity.clamp(20.0, 80.0);

        // Battery drains slowly (0.05-0.15% per step, min 0)
        self.battery -= rng.random_range(0.05..=0.15);
        self.battery = self.battery.max(0.0);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "extrittio_client=info".into()),
        )
        .init();

    let args = Args::parse();

    let device_id = args
        .device_id
        .or_else(read_embedded_device_id)
        .unwrap_or_else(|| {
            eprintln!(
                "Error: no device ID available.\n\
                 Provide --device-id on first run. After OTA the ID is embedded automatically."
            );
            std::process::exit(1);
        });

    info!(
        "Starting device '{}' (telemetry every {}s, heartbeat every {}s)",
        device_id, args.interval, args.heartbeat_interval
    );

    let session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open zenoh session");
    let session = Arc::new(session);

    info!("Zenoh session opened");

    let telemetry_topic = format!("extrittio/devices/{}/telemetry", device_id);
    let heartbeat_topic = format!("extrittio/devices/{}/heartbeat", device_id);
    let shadow_get_topic = format!("extrittio/devices/{}/shadow/get", device_id);
    let shadow_delta_topic = format!("extrittio/devices/{}/shadow/delta", device_id);
    let shadow_report_topic = format!("extrittio/devices/{}/shadow/report", device_id);

    let reported_state: Arc<Mutex<serde_json::Map<String, serde_json::Value>>> =
        Arc::new(Mutex::new(serde_json::Map::new()));

    let firmware_version: Arc<Mutex<String>> = Arc::new(Mutex::new("v1.0.0".to_string()));

    let ota_in_progress: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    let start = Instant::now();

    // Spawn heartbeat task
    let hb_session = session.clone();
    let hb_device_id = device_id.clone();
    let hb_interval = args.heartbeat_interval;
    let hb_firmware = firmware_version.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(hb_interval));
        loop {
            interval.tick().await;

            let fw = hb_firmware.lock().await.clone();
            let heartbeat = DeviceHeartbeat {
                device_id: hb_device_id.clone(),
                timestamp: chrono_now_millis(),
                status: "online".to_string(),
                firmware: fw,
                uptime_seconds: start.elapsed().as_secs() as i64,
            };

            let payload = heartbeat.encode_to_vec();
            if let Err(e) = hb_session.put(&heartbeat_topic, payload).await {
                tracing::warn!("Failed to send heartbeat: {}", e);
            } else {
                info!(
                    "Heartbeat sent (uptime: {}s)",
                    start.elapsed().as_secs()
                );
            }
        }
    });

    // Request any pending shadow delta on startup
    let shadow_get = ShadowGet {
        device_id: device_id.clone(),
    };
    let payload = shadow_get.encode_to_vec();
    if let Err(e) = session.put(&shadow_get_topic, payload).await {
        tracing::warn!("Failed to send ShadowGet: {}", e);
    } else {
        info!("Sent ShadowGet to '{}'", shadow_get_topic);
    }

    // Spawn shadow subscriber task
    let shadow_session = session.clone();
    let shadow_device_id = device_id.clone();
    let shadow_reported = reported_state.clone();
    let shadow_firmware = firmware_version.clone();
    let shadow_ota_flag = ota_in_progress.clone();
    tokio::spawn(async move {
        let subscriber = shadow_session
            .declare_subscriber(&shadow_delta_topic)
            .await
            .expect("Failed to subscribe to shadow/delta");

        info!("Subscribed to '{}'", shadow_delta_topic);

        loop {
            let sample = subscriber.recv_async().await;
            match sample {
                Ok(sample) => {
                    let bytes = sample.payload().to_bytes();
                    match ShadowDelta::decode(bytes.as_ref()) {
                        Ok(delta) => {
                            info!("Shadow delta received: {}", delta.delta_json);

                            // Parse delta JSON and merge into reported state
                            match serde_json::from_str::<serde_json::Value>(&delta.delta_json) {
                                Ok(serde_json::Value::Object(mut delta_map)) => {
                                    // Extract OTA payload before merging
                                    let ota_payload = delta_map.remove("ota");

                                    // Merge remaining keys into reported state
                                    {
                                        let mut state = shadow_reported.lock().await;
                                        for (key, value) in delta_map {
                                            state.insert(key, value);
                                        }
                                    }

                                    // Send report for non-OTA keys
                                    send_shadow_report(
                                        &shadow_device_id,
                                        &shadow_session,
                                        &shadow_report_topic,
                                        &shadow_reported,
                                        delta.version,
                                    )
                                    .await;

                                    // If OTA payload present, spawn OTA handler
                                    if let Some(ota_val) = ota_payload {
                                        // Guard: skip if another OTA is already running
                                        if shadow_ota_flag.compare_exchange(
                                            false, true, Ordering::SeqCst, Ordering::SeqCst,
                                        ).is_err() {
                                            tracing::warn!("OTA: update already in progress, ignoring new delta");
                                            continue;
                                        }

                                        let ota_device_id = shadow_device_id.clone();
                                        let ota_session = shadow_session.clone();
                                        let ota_topic = shadow_report_topic.clone();
                                        let ota_reported = shadow_reported.clone();
                                        let ota_fw = shadow_firmware.clone();
                                        let ota_version = delta.version;
                                        let ota_flag = shadow_ota_flag.clone();
                                        tokio::spawn(async move {
                                            handle_ota(
                                                ota_val,
                                                ota_device_id,
                                                ota_session,
                                                ota_topic,
                                                ota_reported,
                                                ota_fw,
                                                ota_version,
                                            )
                                            .await;
                                            ota_flag.store(false, Ordering::SeqCst);
                                        });
                                    }
                                }
                                Ok(_) => {
                                    tracing::warn!(
                                        "Shadow delta JSON is not an object: {}",
                                        delta.delta_json
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to parse shadow delta JSON: {}",
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to decode ShadowDelta: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Shadow subscriber error: {}", e);
                    break;
                }
            }
        }
    });

    // Telemetry loop (main task)
    let mut sensor = SensorState::new();
    let mut interval = tokio::time::interval(Duration::from_secs(args.interval));

    info!("Sending telemetry to '{}'", telemetry_topic);

    loop {
        interval.tick().await;
        sensor.step();

        let telemetry = DeviceTelemetry {
            device_id: device_id.clone(),
            timestamp: chrono_now_millis(),
            temperature: sensor.temperature,
            humidity: sensor.humidity,
            battery_level: sensor.battery,
            metadata: Default::default(),
        };

        let payload = telemetry.encode_to_vec();
        if let Err(e) = session.put(&telemetry_topic, payload).await {
            tracing::warn!("Failed to send telemetry: {}", e);
        } else {
            info!(
                "Telemetry: temp={:.1}°C humidity={:.1}% battery={:.1}%",
                sensor.temperature, sensor.humidity, sensor.battery
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Shadow helpers
// ---------------------------------------------------------------------------

async fn send_shadow_report(
    device_id: &str,
    session: &zenoh::Session,
    topic: &str,
    reported_state: &Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    version: i64,
) {
    let state = reported_state.lock().await;
    let state_json = serde_json::to_string(&*state).unwrap_or_else(|_| "{}".to_string());
    info!("Applied. Reported state: {}", state_json);

    let report = ShadowReport {
        device_id: device_id.to_string(),
        timestamp: chrono_now_millis(),
        state_json,
        version,
    };

    let payload = report.encode_to_vec();
    if let Err(e) = session.put(topic, payload).await {
        tracing::warn!("Failed to send ShadowReport: {}", e);
    } else {
        info!("ShadowReport sent (version: {})", version);
    }
}

async fn report_ota_status(
    reported_state: &Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    device_id: &str,
    session: &zenoh::Session,
    topic: &str,
    version: i64,
    fw_version: &str,
    fw_update_id: Option<i64>,
    status: &str,
    error: Option<&str>,
) {
    let mut ota_obj = serde_json::json!({
        "status": status,
        "firmware_version": fw_version,
    });
    if let Some(id) = fw_update_id {
        ota_obj["firmware_update_id"] = serde_json::json!(id);
    }
    if let Some(err) = error {
        ota_obj["error"] = serde_json::json!(err);
    }

    {
        let mut state = reported_state.lock().await;
        state.insert("ota".to_string(), ota_obj);
    }

    send_shadow_report(device_id, session, topic, reported_state, version).await;
}

// ---------------------------------------------------------------------------
// OTA handler (real binary replacement for Linux)
// ---------------------------------------------------------------------------

async fn handle_ota(
    ota_payload: serde_json::Value,
    device_id: String,
    session: Arc<zenoh::Session>,
    report_topic: String,
    reported_state: Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    firmware_version: Arc<Mutex<String>>,
    shadow_version: i64,
) {
    // Parse required fields
    let fw_version = match ota_payload.get("firmware_version").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => {
            tracing::warn!("OTA payload missing firmware_version");
            return;
        }
    };
    let fw_url = match ota_payload.get("firmware_url").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => {
            tracing::warn!("OTA payload missing firmware_url");
            return;
        }
    };
    let fw_update_id = ota_payload
        .get("firmware_update_id")
        .and_then(|v| v.as_i64());
    let expected_sha256 = ota_payload
        .get("sha256")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Check if we're already running the requested version
    {
        let current = firmware_version.lock().await;
        if current.contains(&fw_version) {
            info!("OTA: already running v{}, skipping", fw_version);
            report_ota_status(
                &reported_state, &device_id, &session, &report_topic, shadow_version,
                &fw_version, fw_update_id, "success", None,
            )
            .await;
            return;
        }
    }

    // -- Report "downloading" --
    info!("OTA: downloading firmware v{} from {}", fw_version, fw_url);
    report_ota_status(
        &reported_state, &device_id, &session, &report_topic, shadow_version,
        &fw_version, fw_update_id, "downloading", None,
    )
    .await;

    // -- Download with timeout --
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
                    &reported_state, &device_id, &session, &report_topic, shadow_version,
                    &fw_version, fw_update_id, "failed", Some(&err),
                )
                .await;
                return;
            }
            match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    let err = format!("download read error: {}", e);
                    tracing::warn!("OTA: {}", err);
                    report_ota_status(
                        &reported_state, &device_id, &session, &report_topic, shadow_version,
                        &fw_version, fw_update_id, "failed", Some(&err),
                    )
                    .await;
                    return;
                }
            }
        }
        Err(e) => {
            let err = format!("download error: {}", e);
            tracing::warn!("OTA: {}", err);
            report_ota_status(
                &reported_state, &device_id, &session, &report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            )
            .await;
            return;
        }
    };

    info!("OTA: downloaded {} bytes", bytes.len());

    // -- Verify SHA-256 (if provided) --
    if let Some(ref expected) = expected_sha256 {
        report_ota_status(
            &reported_state, &device_id, &session, &report_topic, shadow_version,
            &fw_version, fw_update_id, "verifying", None,
        )
        .await;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = format!("{:x}", hasher.finalize());

        if actual.to_lowercase() != expected.to_lowercase() {
            let err = format!("hash mismatch: expected={} got={}", expected, actual);
            tracing::warn!("OTA: {}", err);
            report_ota_status(
                &reported_state, &device_id, &session, &report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            )
            .await;
            return;
        }
        info!("OTA: SHA-256 verified");
    }

    // -- Patch device ID into the downloaded binary --
    let mut firmware_bytes = bytes.to_vec();
    if patch_device_id_in_binary(&mut firmware_bytes, &device_id) {
        info!("OTA: embedded device ID '{}' into firmware", device_id);
    } else {
        tracing::warn!("OTA: device ID marker not found in firmware — ID will not be embedded");
    }

    // -- Install: replace current executable --
    report_ota_status(
        &reported_state, &device_id, &session, &report_topic, shadow_version,
        &fw_version, fw_update_id, "installing", None,
    )
    .await;

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            let err = format!("cannot resolve current executable path: {}", e);
            tracing::warn!("OTA: {}", err);
            report_ota_status(
                &reported_state, &device_id, &session, &report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            )
            .await;
            return;
        }
    };

    // Write to a temp file next to the current binary, then atomically rename
    let tmp_path = current_exe.with_extension("ota_tmp");

    if let Err(e) = tokio::fs::write(&tmp_path, &firmware_bytes).await {
        let err = format!("failed to write firmware to {}: {}", tmp_path.display(), e);
        tracing::warn!("OTA: {}", err);
        let _ = tokio::fs::remove_file(&tmp_path).await;
        report_ota_status(
            &reported_state, &device_id, &session, &report_topic, shadow_version,
            &fw_version, fw_update_id, "failed", Some(&err),
        )
        .await;
        return;
    }

    // Set executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = tokio::fs::set_permissions(
            &tmp_path,
            std::fs::Permissions::from_mode(0o755),
        )
        .await
        {
            let err = format!("failed to set permissions: {}", e);
            tracing::warn!("OTA: {}", err);
            let _ = tokio::fs::remove_file(&tmp_path).await;
            report_ota_status(
                &reported_state, &device_id, &session, &report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            )
            .await;
            return;
        }
    }

    // Atomic rename: tmp -> current executable
    if let Err(e) = tokio::fs::rename(&tmp_path, &current_exe).await {
        let err = format!("failed to replace binary: {}", e);
        tracing::warn!("OTA: {}", err);
        let _ = tokio::fs::remove_file(&tmp_path).await;
        report_ota_status(
            &reported_state, &device_id, &session, &report_topic, shadow_version,
            &fw_version, fw_update_id, "failed", Some(&err),
        )
        .await;
        return;
    }

    // -- Success: report and exit so the process manager (systemd) restarts us --
    {
        let mut fw = firmware_version.lock().await;
        *fw = format!("v{}", fw_version);
    }

    report_ota_status(
        &reported_state, &device_id, &session, &report_topic, shadow_version,
        &fw_version, fw_update_id, "success", None,
    )
    .await;

    info!(
        "OTA: binary replaced at {}. Exiting for process manager restart.",
        current_exe.display()
    );

    // Give Zenoh time to flush the success report
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Exit with code 0 — systemd Restart=always will relaunch the new binary
    std::process::exit(0);
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn chrono_now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}
