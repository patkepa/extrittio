use std::time::Duration;

use clap::Parser;
use extrittio_client_runtime::{
    EMBED_MARKER_LEN, EMBED_SLOT_LEN, EventSource, NativeClientConfig, contract::ContractEvent,
    run_native_client,
};
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
    contract: String,
    #[arg(long, default_value_t = 5)]
    interval: u64,
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

impl EventSource for SimulatedTelemetry {
    fn sample(&mut self) -> ContractEvent {
        let mut rng = rand::rng();
        self.sensor.step(
            rng.random_range(-0.5..=0.5),
            rng.random_range(-1.0..=1.0),
            rng.random_range(0.05..=0.15),
        );
        ContractEvent::new(
            "environment",
            serde_json::json!({
                "temperature": self.sensor.temperature,
                "humidity": self.sensor.humidity,
                "batteryLevel": self.sensor.battery
            }),
        )
    }
}

#[tokio::main]
async fn main() {
    extrittio_sdk::native_ota::watchdog_entry();
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
        connect: args.connect,
        ca_cert: args.ca_cert,
        client_cert: args.client_cert,
        client_key: args.client_key,
        client_name: "Linux client",
    };
    run_native_client(config, SimulatedTelemetry::default(), &DEVICE_ID_EMBED).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_is_required_and_sample_is_a_native_stream_payload() {
        assert!(Args::try_parse_from(["extrittio-client"]).is_err());
        assert!(Args::try_parse_from(["extrittio-client", "--contract", "contract.json"]).is_ok());
        let event = SimulatedTelemetry::default().sample();
        assert_eq!(event.stream_key, "environment");
        assert_eq!(event.payload.as_object().unwrap().len(), 3);
        for key in ["temperature", "humidity", "batteryLevel"] {
            assert!(event.payload[key].is_number());
        }
    }
}
