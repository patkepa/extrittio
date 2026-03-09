use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use extrittio_proto::extrittio::{
    DeviceHeartbeat, DeviceTelemetry, ShadowDelta, ShadowGet, ShadowReport,
};
use log::info;
use prost::Message;
use rand::Rng;

// ── Configuration ───────────────────────────────────────────────────
// Edit these constants or wire them to NVS / menuconfig for production.
const DEVICE_ID: &str = "esp32-001";
const WIFI_SSID: &str = "your-wifi-ssid";
const WIFI_PASS: &str = "your-wifi-password";
const TELEMETRY_INTERVAL_SECS: u64 = 5;
const HEARTBEAT_INTERVAL_SECS: u64 = 30;

// Zenoh router endpoint — set to the IP of the machine running the backend.
// Leave empty to use Zenoh multicast scouting (same LAN only).
const ZENOH_CONNECT: &str = "tcp/192.0.2.100:7447";

// ── Sensor simulation ──────────────────────────────────────────────

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
        let mut rng = rand::thread_rng();

        // Random walk ±0.5 °C, clamped 15–30
        self.temperature += rng.gen_range(-0.5..=0.5);
        self.temperature = self.temperature.clamp(15.0, 30.0);

        // Random walk ±1 %, clamped 20–80
        self.humidity += rng.gen_range(-1.0..=1.0);
        self.humidity = self.humidity.clamp(20.0, 80.0);

        // Battery drain 0.05–0.15 % per step
        self.battery -= rng.gen_range(0.05..=0.15);
        self.battery = self.battery.max(0.0);
    }
}

// ── Entry point ─────────────────────────────────────────────────────

fn main() {
    // Bind the ESP-IDF patches and initialise the default logger.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

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

    let telemetry_topic = format!("extrittio/devices/{}/telemetry", DEVICE_ID);
    let heartbeat_topic = format!("extrittio/devices/{}/heartbeat", DEVICE_ID);
    let shadow_get_topic = format!("extrittio/devices/{}/shadow/get", DEVICE_ID);
    let shadow_delta_topic = format!("extrittio/devices/{}/shadow/delta", DEVICE_ID);
    let shadow_report_topic = format!("extrittio/devices/{}/shadow/report", DEVICE_ID);
    let start = Instant::now();

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

                            // Echo the delta back as reported state (simple embedded approach)
                            let report = ShadowReport {
                                device_id: DEVICE_ID.to_string(),
                                timestamp: now_millis(),
                                state_json: delta.delta_json,
                                version: delta.version,
                            };

                            match shadow_session
                                .put(&shadow_report_key, report.encode_to_vec())
                                .wait()
                            {
                                Ok(_) => info!("Shadow report sent (version={})", delta.version),
                                Err(e) => log::warn!("Failed to send shadow report: {e}"),
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
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

        let heartbeat = DeviceHeartbeat {
            device_id: DEVICE_ID.to_string(),
            timestamp: now_millis(),
            status: "online".to_string(),
            firmware: "v1.0.0-esp32".to_string(),
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
        sensor.step();

        let telemetry = DeviceTelemetry {
            device_id: DEVICE_ID.to_string(),
            timestamp: now_millis(),
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

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}
