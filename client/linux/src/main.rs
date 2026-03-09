use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use extrittio_proto::extrittio::{
    DeviceHeartbeat, DeviceTelemetry, ShadowDelta, ShadowGet, ShadowReport,
};
use prost::Message;
use rand::Rng;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Parser)]
#[command(name = "extrittio-client", about = "Simulated IoT device client")]
struct Args {
    /// Device ID (must match a device registered in the backend)
    #[arg(long, default_value = "dev-001")]
    device_id: String,

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

    info!(
        "Starting simulated device '{}' (telemetry every {}s, heartbeat every {}s)",
        args.device_id, args.interval, args.heartbeat_interval
    );

    let session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open zenoh session");
    let session = Arc::new(session);

    info!("Zenoh session opened");

    let telemetry_topic = format!("extrittio/devices/{}/telemetry", args.device_id);
    let heartbeat_topic = format!("extrittio/devices/{}/heartbeat", args.device_id);
    let shadow_get_topic = format!("extrittio/devices/{}/shadow/get", args.device_id);
    let shadow_delta_topic = format!("extrittio/devices/{}/shadow/delta", args.device_id);
    let shadow_report_topic = format!("extrittio/devices/{}/shadow/report", args.device_id);

    let reported_state: Arc<Mutex<serde_json::Map<String, serde_json::Value>>> =
        Arc::new(Mutex::new(serde_json::Map::new()));

    let start = Instant::now();

    // Spawn heartbeat task
    let hb_session = session.clone();
    let hb_device_id = args.device_id.clone();
    let hb_interval = args.heartbeat_interval;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(hb_interval));
        loop {
            interval.tick().await;

            let heartbeat = DeviceHeartbeat {
                device_id: hb_device_id.clone(),
                timestamp: chrono_now_millis(),
                status: "online".to_string(),
                firmware: "v1.0.0".to_string(),
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
        device_id: args.device_id.clone(),
    };
    let payload = shadow_get.encode_to_vec();
    if let Err(e) = session.put(&shadow_get_topic, payload).await {
        tracing::warn!("Failed to send ShadowGet: {}", e);
    } else {
        info!("Sent ShadowGet to '{}'", shadow_get_topic);
    }

    // Spawn shadow subscriber task
    let shadow_session = session.clone();
    let shadow_device_id = args.device_id.clone();
    let shadow_reported = reported_state.clone();
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
                                Ok(serde_json::Value::Object(delta_map)) => {
                                    let mut state = shadow_reported.lock().await;
                                    for (key, value) in delta_map {
                                        state.insert(key, value);
                                    }

                                    let state_json = serde_json::to_string(&*state)
                                        .unwrap_or_else(|_| "{}".to_string());
                                    info!("Applied. Reported state: {}", state_json);

                                    let report = ShadowReport {
                                        device_id: shadow_device_id.clone(),
                                        timestamp: chrono_now_millis(),
                                        state_json,
                                        version: delta.version,
                                    };

                                    let payload = report.encode_to_vec();
                                    if let Err(e) = shadow_session
                                        .put(&shadow_report_topic, payload)
                                        .await
                                    {
                                        tracing::warn!("Failed to send ShadowReport: {}", e);
                                    } else {
                                        info!("ShadowReport sent (version: {})", delta.version);
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
            device_id: args.device_id.clone(),
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

fn chrono_now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}
