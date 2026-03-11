use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use extrittio_common::extrittio::{
    DeviceHeartbeat, DeviceTelemetry, ShadowDelta, ShadowGet, ShadowReport,
};
use extrittio_common::{device_status, ota::fields as ota_fields, topics};
use extrittio_sdk::sensor::SensorState;
use log::info;
use prost::Message;
use rand::Rng;
use sha2::{Digest, Sha256};

// ── Configuration ───────────────────────────────────────────────────
// Edit these constants or wire them to NVS / menuconfig for production.
const DEVICE_ID: &str = "esp32-001";
const WIFI_SSID: &str = "your-wifi-ssid";
const WIFI_PASS: &str = "your-wifi-password";
const TELEMETRY_INTERVAL_SECS: u64 = 5;
const HEARTBEAT_INTERVAL_SECS: u64 = 30;
const FIRMWARE_VERSION: &str = "v1.0.0-esp32";

// Zenoh router endpoint — set to the IP of the machine running the backend.
// Leave empty to use Zenoh multicast scouting (same LAN only).
const ZENOH_CONNECT: &str = "tcp/192.0.2.100:7447";


// ── Entry point ─────────────────────────────────────────────────────

fn main() {
    // Bind the ESP-IDF patches and initialise the default logger.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // Mark the current OTA app as valid (prevents rollback on next boot).
    // This is a no-op if we booted from the factory partition.
    unsafe {
        esp_idf_svc::sys::esp_ota_mark_app_valid_and_cancel_rollback();
    }

    info!("Extrittio ESP32 client starting (device '{}')", DEVICE_ID);

    // ── Hardware / system init ──────────────────────────────────────
    let peripherals = Peripherals::take().expect("Failed to take peripherals");
    let sysloop = EspSystemEventLoop::take().expect("Failed to take event loop");
    let nvs = EspDefaultNvsPartition::take().expect("Failed to take NVS partition");

    // ── WiFi ────────────────────────────────────────────────────────
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs)).expect("Failed to create WiFi"),
        sysloop,
    )
    .expect("Failed to wrap WiFi");

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().expect("SSID too long"),
        password: WIFI_PASS.try_into().expect("Password too long"),
        ..Default::default()
    }))
    .expect("Failed to set WiFi configuration");

    wifi.start().expect("Failed to start WiFi");
    wifi.connect().expect("Failed to connect to WiFi");
    wifi.wait_netif_up().expect("Failed to bring up network interface");

    info!("WiFi connected");

    // ── Zenoh ───────────────────────────────────────────────────────
    let mut zenoh_cfg = zenoh::Config::default();
    if !ZENOH_CONNECT.is_empty() {
        zenoh_cfg
            .connect
            .endpoints
            .set(vec![ZENOH_CONNECT.parse().expect("Bad Zenoh endpoint")])
            .expect("Failed to set Zenoh connect endpoints");
    }

    // .wait() is the blocking equivalent of .await for Zenoh builders.
    let session = zenoh::open(zenoh_cfg)
        .wait()
        .expect("Failed to open Zenoh session");
    let session = Arc::new(session);

    info!("Zenoh session opened");

    let telemetry_topic = topics::telemetry(DEVICE_ID);
    let heartbeat_topic = topics::heartbeat(DEVICE_ID);
    let shadow_get_topic = topics::shadow_get(DEVICE_ID);
    let shadow_delta_topic = topics::shadow_delta(DEVICE_ID);
    let shadow_report_topic = topics::shadow_report(DEVICE_ID);
    let start = Instant::now();

    // Shared state
    let reported_state: Arc<Mutex<serde_json::Map<String, serde_json::Value>>> =
        Arc::new(Mutex::new(serde_json::Map::new()));
    let firmware_version: Arc<Mutex<String>> =
        Arc::new(Mutex::new(FIRMWARE_VERSION.to_string()));

    // ── Request pending shadow delta on startup ──────────────────────
    let shadow_get = ShadowGet {
        device_id: DEVICE_ID.to_string(),
    };
    match session.put(&shadow_get_topic, shadow_get.encode_to_vec()).wait() {
        Ok(_) => info!("Shadow get request sent to '{shadow_get_topic}'"),
        Err(e) => log::warn!("Failed to send shadow get request: {e}"),
    }

    // ── Shadow subscriber thread ─────────────────────────────────────
    let shadow_session = session.clone();
    let shadow_report_key = shadow_report_topic.clone();
    let shadow_reported = reported_state.clone();
    let shadow_firmware = firmware_version.clone();
    thread::spawn(move || {
        let subscriber = shadow_session
            .declare_subscriber(&shadow_delta_topic)
            .wait()
            .expect("Failed to declare shadow delta subscriber");

        info!("Shadow subscriber listening on '{shadow_delta_topic}'");

        loop {
            match subscriber.recv() {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    match ShadowDelta::decode(payload.as_ref()) {
                        Ok(delta) => {
                            info!(
                                "Shadow delta received: version={}, delta_json={}",
                                delta.version, delta.delta_json
                            );

                            // Parse delta JSON and merge into reported state
                            match serde_json::from_str::<serde_json::Value>(&delta.delta_json) {
                                Ok(serde_json::Value::Object(mut delta_map)) => {
                                    // Extract OTA payload before merging
                                    let ota_payload = delta_map.remove(ota_fields::SHADOW_KEY);

                                    // Merge remaining keys into reported state
                                    {
                                        let mut state = shadow_reported.lock().unwrap();
                                        for (key, value) in delta_map {
                                            state.insert(key, value);
                                        }
                                    }

                                    // Send report for non-OTA keys
                                    send_shadow_report(
                                        &shadow_session,
                                        &shadow_report_key,
                                        &shadow_reported,
                                        delta.version,
                                    );

                                    // If OTA payload present, handle it
                                    // Runs synchronously in this thread to prevent concurrent OTA
                                    if let Some(ota_val) = ota_payload {
                                        handle_ota(
                                            ota_val,
                                            &shadow_session,
                                            &shadow_report_key,
                                            &shadow_reported,
                                            &shadow_firmware,
                                            delta.version,
                                        );
                                    }
                                }
                                Ok(_) => {
                                    log::warn!(
                                        "Shadow delta JSON is not an object: {}",
                                        delta.delta_json
                                    );
                                }
                                Err(e) => {
                                    log::warn!("Failed to parse shadow delta JSON: {e}");
                                }
                            }
                        }
                        Err(e) => log::warn!("Failed to decode shadow delta: {e}"),
                    }
                }
                Err(e) => {
                    log::warn!("Shadow subscriber recv error: {e}");
                    break;
                }
            }
        }
    });

    // ── Heartbeat thread ────────────────────────────────────────────
    let hb_session = session.clone();
    let hb_topic = heartbeat_topic.clone();
    let hb_firmware = firmware_version.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

        let fw = hb_firmware.lock().unwrap().clone();
        let heartbeat = DeviceHeartbeat {
            device_id: DEVICE_ID.to_string(),
            timestamp: extrittio_sdk::time::extrittio_sdk::time::now_millis(),
            status: device_status::ONLINE.to_string(),
            firmware: fw,
            uptime_seconds: start.elapsed().as_secs() as i64,
        };

        let payload = heartbeat.encode_to_vec();
        match hb_session.put(&hb_topic, payload).wait() {
            Ok(_) => info!("Heartbeat sent (uptime: {}s)", start.elapsed().as_secs()),
            Err(e) => log::warn!("Failed to send heartbeat: {e}"),
        }
    });

    // ── Telemetry loop (main thread) ────────────────────────────────
    let mut sensor = SensorState::new();
    info!("Sending telemetry to '{telemetry_topic}'");

    // Keep wifi alive — the binding must not be dropped.
    let _wifi = wifi;

    loop {
        thread::sleep(Duration::from_secs(TELEMETRY_INTERVAL_SECS));
        let mut rng = rand::thread_rng();
        sensor.step(
            rng.gen_range(-0.5..=0.5),
            rng.gen_range(-1.0..=1.0),
            rng.gen_range(0.05..=0.15),
        );

        let telemetry = DeviceTelemetry {
            device_id: DEVICE_ID.to_string(),
            timestamp: extrittio_sdk::time::now_millis(),
            temperature: sensor.temperature,
            humidity: sensor.humidity,
            battery_level: sensor.battery,
            metadata: Default::default(),
        };

        let payload = telemetry.encode_to_vec();
        match session.put(&telemetry_topic, payload).wait() {
            Ok(_) => info!(
                "Telemetry: temp={:.1}°C humidity={:.1}% battery={:.1}%",
                sensor.temperature, sensor.humidity, sensor.battery
            ),
            Err(e) => log::warn!("Failed to send telemetry: {e}"),
        }
    }
}

// ── Shadow helpers ──────────────────────────────────────────────────

fn send_shadow_report(
    session: &zenoh::Session,
    topic: &str,
    reported_state: &Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    version: i64,
) {
    let state = reported_state.lock().unwrap();
    let state_json = serde_json::to_string(&*state).unwrap_or_else(|_| "{}".to_string());

    let report = ShadowReport {
        device_id: DEVICE_ID.to_string(),
        timestamp: extrittio_sdk::time::now_millis(),
        state_json,
        version,
    };

    match session.put(topic, report.encode_to_vec()).wait() {
        Ok(_) => info!("Shadow report sent (version={})", version),
        Err(e) => log::warn!("Failed to send shadow report: {e}"),
    }
}

fn report_ota_status(
    reported_state: &Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    session: &zenoh::Session,
    topic: &str,
    version: i64,
    fw_version: &str,
    fw_update_id: Option<i64>,
    status: &str,
    error: Option<&str>,
) {
    let ota_obj = extrittio_sdk::ota::build_status_json(status, fw_version, fw_update_id, error);

    {
        let mut state = reported_state.lock().unwrap();
        state.insert(ota_fields::SHADOW_KEY.to_string(), ota_obj);
    }

    send_shadow_report(session, topic, reported_state, version);
}

// ── OTA handler (real ESP-IDF OTA) ──────────────────────────────────

fn handle_ota(
    ota_payload: serde_json::Value,
    session: &zenoh::Session,
    report_topic: &str,
    reported_state: &Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    firmware_version: &Arc<Mutex<String>>,
    shadow_version: i64,
) {
    // Parse required fields
    let parsed = match extrittio_sdk::ota::OtaPayload::from_json(&ota_payload) {
        Some(p) => p,
        None => {
            log::warn!("OTA payload missing required fields");
            return;
        }
    };
    let fw_version = parsed.firmware_version;
    let fw_url = parsed.firmware_url;
    let fw_update_id = parsed.firmware_update_id;
    let expected_sha256 = parsed.sha256;

    // Check if we're already running the requested version
    {
        let current = firmware_version.lock().unwrap();
        if current.contains(&fw_version) {
            info!("OTA: already running v{}, skipping", fw_version);
            report_ota_status(
                reported_state, session, report_topic, shadow_version,
                &fw_version, fw_update_id, "success", None,
            );
            return;
        }
    }

    // -- Report "downloading" --
    info!("OTA: downloading firmware v{} from {}", fw_version, fw_url);
    report_ota_status(
        reported_state, session, report_topic, shadow_version,
        &fw_version, fw_update_id, "downloading", None,
    );

    // -- Initialize ESP-IDF OTA --
    let mut ota = match esp_idf_svc::ota::EspOta::new() {
        Ok(o) => o,
        Err(e) => {
            let err = format!("OTA init error: {}", e);
            log::warn!("OTA: {}", err);
            report_ota_status(
                reported_state, session, report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            );
            return;
        }
    };

    let mut ota_update = match ota.initiate_update() {
        Ok(u) => u,
        Err(e) => {
            let err = format!("OTA update init error: {}", e);
            log::warn!("OTA: {}", err);
            report_ota_status(
                reported_state, session, report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            );
            return;
        }
    };

    // -- Download firmware via ESP-IDF HTTP client, stream to OTA partition --
    let http_config = esp_idf_svc::http::client::Configuration {
        buffer_size: Some(4096),
        buffer_size_tx: Some(2048),
        ..Default::default()
    };

    let mut http_conn = match esp_idf_svc::http::client::EspHttpConnection::new(&http_config) {
        Ok(conn) => conn,
        Err(e) => {
            let err = format!("HTTP init error: {}", e);
            log::warn!("OTA: {}", err);
            let _ = ota_update.abort();
            report_ota_status(
                reported_state, session, report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            );
            return;
        }
    };

    use embedded_svc::http::client::Client;
    let mut http_client = Client::wrap(http_conn);

    let request = match http_client.get(&fw_url) {
        Ok(req) => req,
        Err(e) => {
            let err = format!("HTTP request error: {}", e);
            log::warn!("OTA: {}", err);
            let _ = ota_update.abort();
            report_ota_status(
                reported_state, session, report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            );
            return;
        }
    };

    let mut response = match request.submit() {
        Ok(resp) => resp,
        Err(e) => {
            let err = format!("HTTP submit error: {}", e);
            log::warn!("OTA: {}", err);
            let _ = ota_update.abort();
            report_ota_status(
                reported_state, session, report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            );
            return;
        }
    };

    let status = response.status();
    if !(200..300).contains(&(status as i32)) {
        let err = format!("HTTP {}", status);
        log::warn!("OTA: download failed: {}", err);
        let _ = ota_update.abort();
        report_ota_status(
            reported_state, session, report_topic, shadow_version,
            &fw_version, fw_update_id, "failed", Some(&err),
        );
        return;
    }

    // Read body in chunks, write to OTA partition, hash simultaneously
    let mut hasher = Sha256::new();
    let mut total_bytes: usize = 0;
    let mut buf = [0u8; 4096];

    use embedded_svc::io::Read;
    loop {
        match response.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                hasher.update(&buf[..n]);
                if let Err(e) = ota_update.write(&buf[..n]) {
                    let err = format!("OTA write error: {}", e);
                    log::warn!("OTA: {}", err);
                    let _ = ota_update.abort();
                    report_ota_status(
                        reported_state, session, report_topic, shadow_version,
                        &fw_version, fw_update_id, "failed", Some(&err),
                    );
                    return;
                }
                total_bytes += n;
            }
            Err(e) => {
                let err = format!("HTTP read error: {}", e);
                log::warn!("OTA: {}", err);
                let _ = ota_update.abort();
                report_ota_status(
                    reported_state, session, report_topic, shadow_version,
                    &fw_version, fw_update_id, "failed", Some(&err),
                );
                return;
            }
        }
    }

    info!("OTA: downloaded and wrote {} bytes to OTA partition", total_bytes);

    // -- Verify SHA-256 (if provided) --
    if let Some(ref expected) = expected_sha256 {
        report_ota_status(
            reported_state, session, report_topic, shadow_version,
            &fw_version, fw_update_id, "verifying", None,
        );

        let actual = format!("{:x}", hasher.finalize());
        if actual.to_lowercase() != expected.to_lowercase() {
            let err = format!("hash mismatch: expected={} got={}", expected, actual);
            log::warn!("OTA: {}", err);
            let _ = ota_update.abort();
            report_ota_status(
                reported_state, session, report_topic, shadow_version,
                &fw_version, fw_update_id, "failed", Some(&err),
            );
            return;
        }
        info!("OTA: SHA-256 verified");
    }

    // -- Report "installing" --
    report_ota_status(
        reported_state, session, report_topic, shadow_version,
        &fw_version, fw_update_id, "installing", None,
    );

    // Complete OTA — marks the new partition as bootable
    if let Err(e) = ota_update.complete() {
        let err = format!("OTA complete error: {}", e);
        log::warn!("OTA: {}", err);
        report_ota_status(
            reported_state, session, report_topic, shadow_version,
            &fw_version, fw_update_id, "failed", Some(&err),
        );
        return;
    }

    // Report "success" BEFORE rebooting
    report_ota_status(
        reported_state, session, report_topic, shadow_version,
        &fw_version, fw_update_id, "success", None,
    );
    info!("OTA: firmware v{} installed, rebooting...", fw_version);

    // Give Zenoh enough time to flush the shadow report before reboot
    thread::sleep(Duration::from_secs(2));

    // Reboot into the new firmware
    esp_idf_svc::hal::reset::restart();
}

