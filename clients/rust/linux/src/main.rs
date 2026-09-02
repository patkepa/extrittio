use std::collections::HashMap;
use std::time::Duration;

use clap::Parser;
use extrittio_client_runtime::{
    EMBED_MARKER_LEN, EMBED_SLOT_LEN, NativeClientConfig, TelemetrySource, contract::ContractEvent,
    run_native_client,
};
use extrittio_common::extrittio::DeviceTelemetry;
use extrittio_sdk::sensor::SensorState;
use rand::Rng;

#[used]
#[unsafe(no_mangle)]
pub static DEVICE_ID_EMBED: [u8; EMBED_SLOT_LEN] = {
    let marker = b"<<EXTRITTIO_DEVICE_ID>>";
    let mut slot = [0_u8; EMBED_SLOT_LEN];
    let mut index = 0;
    while index < EMBED_MARKER_LEN {
        slot[index] = marker[index];
        index += 1;
    }
    slot
};

#[derive(Parser)]
#[command(name = "extrittio-client", about = "Simulated IoT device client")]
struct Args {
    #[arg(long)]
    device_id: Option<String>,
    /// Provisioned contract JSON downloaded from the Extrittio device contract endpoint.
    #[arg(long)]
    contract: Option<String>,
    #[arg(long, default_value_t = 5)]
    interval: u64,
    #[arg(long, default_value_t = 30)]
    heartbeat_interval: u64,
    #[arg(long)]
    connect: Option<String>,
    #[arg(long)]
    ca_cert: Option<String>,
    #[arg(long)]
    client_cert: Option<String>,
    #[arg(long)]
    client_key: Option<String>,
}

struct SimulatedTelemetry {
    sensor: SensorState,
}

impl Default for SimulatedTelemetry {
    fn default() -> Self {
        Self {
            sensor: SensorState::new(),
        }
    }
}

impl TelemetrySource for SimulatedTelemetry {
    fn sample(&mut self, device_id: &str, timestamp: i64) -> DeviceTelemetry {
        let mut rng = rand::rng();
        self.sensor.step(
            rng.random_range(-0.5..=0.5),
            rng.random_range(-1.0..=1.0),
            rng.random_range(0.05..=0.15),
        );
        DeviceTelemetry {
            device_id: device_id.to_string(),
            timestamp,
            temperature: self.sensor.temperature,
            humidity: self.sensor.humidity,
            battery_level: self.sensor.battery,
            metadata: HashMap::default(),
            latitude: 0.0,
            longitude: 0.0,
            speed: 0.0,
            altitude: 0.0,
            heading: 0.0,
            has_location: false,
        }
    }

    fn summary(&self, _telemetry: &DeviceTelemetry) -> String {
        format!(
            "Telemetry: temp={:.1}°C humidity={:.1}% battery={:.1}%",
            self.sensor.temperature, self.sensor.humidity, self.sensor.battery
        )
    }

    fn contract_event(&self, telemetry: &DeviceTelemetry) -> Option<ContractEvent> {
        Some(ContractEvent::new(
            "environment",
            serde_json::json!({
                "temperature": telemetry.temperature,
                "humidity": telemetry.humidity,
                "batteryLevel": telemetry.battery_level
            }),
        ))
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
    let config = NativeClientConfig {
        device_id: args.device_id,
        contract_path: args.contract,
        telemetry_interval: Duration::from_secs(args.interval),
        heartbeat_interval: Duration::from_secs(args.heartbeat_interval),
        connect: args.connect,
        ca_cert: args.ca_cert,
        client_cert: args.client_cert,
        client_key: args.client_key,
        client_name: "Linux client",
    };
    run_native_client(config, SimulatedTelemetry::default(), &DEVICE_ID_EMBED).await;
}
