use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use extrittio_proto::extrittio::{DeviceHeartbeat, DeviceTelemetry};
use prost::Message;
use rand::Rng;
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
