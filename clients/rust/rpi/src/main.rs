mod metrics;

use std::time::Duration;

use clap::Parser;
use extrittio_client_runtime::{
    EMBED_MARKER_LEN, EMBED_SLOT_LEN, EventSource, NativeClientConfig, contract::ContractEvent,
    run_native_client,
};

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
#[command(name = "extrittio-rpi", about = "Raspberry Pi system telemetry client")]
struct Args {
    #[arg(long)]
    device_id: Option<String>,
    /// Provisioned contract JSON downloaded from the Extrittio device contract endpoint.
    #[arg(long)]
    contract: String,
    /// Validate the provisioned contract and a local sample, then exit without networking.
    #[arg(long)]
    check_contract: bool,
    #[arg(long, default_value_t = 10)]
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

struct SystemTelemetry {
    previous_cpu: Option<metrics::CpuSnapshot>,
}

impl SystemTelemetry {
    fn new() -> Self {
        Self {
            previous_cpu: metrics::CpuSnapshot::take(),
        }
    }
}

impl EventSource for SystemTelemetry {
    fn sample(&mut self) -> ContractEvent {
        let current = metrics::CpuSnapshot::take();
        let cpu_usage = current
            .as_ref()
            .zip(self.previous_cpu.as_ref())
            .map(|(current, previous)| current.usage_since(previous));
        self.previous_cpu = current;
        let metadata = metrics::extended_metrics();
        let number = |key: &str| {
            metadata
                .get(key)
                .and_then(|value| value.parse::<f64>().ok())
        };
        let mut payload = serde_json::json!({
            "cpuTemperature": metrics::cpu_temperature(),
            "cpuUsagePercent": cpu_usage,
            "memoryUsagePercent": metrics::memory_usage_percent(),
            "load1m": number("load_1m"),
            "load5m": number("load_5m"),
            "load15m": number("load_15m")
        });
        payload
            .as_object_mut()
            .expect("object payload")
            .retain(|_, value| !value.is_null());
        ContractEvent::new("system", payload)
    }
}

#[tokio::main]
async fn main() {
    extrittio_client_runtime::watchdog_entry();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "extrittio_rpi=info".into()),
        )
        .init();
    let args = Args::parse();
    if args.check_contract {
        let result = async {
            let contract =
                extrittio_client_runtime::contract::ProvisionedContract::load(&args.contract)
                    .await?;
            if let Some(device_id) = args.device_id.as_deref() {
                contract.validate_device_id(device_id)?;
            }
            if let Some(embedded_id) =
                extrittio_client_runtime::read_embedded_device_id(&DEVICE_ID_EMBED)
            {
                contract.validate_device_id(&embedded_id)?;
            }
            contract.zenoh_endpoint()?;
            contract.encode_event(&SystemTelemetry::new().sample(), chrono::Utc::now())?;
            Ok::<(), extrittio_client_runtime::contract::ProvisionedContractError>(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("Contract validation failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    let config = NativeClientConfig {
        device_id: args.device_id,
        contract_path: args.contract,
        telemetry_interval: Duration::from_secs(args.interval),
        connect: args.connect,
        ca_cert: args.ca_cert,
        client_cert: args.client_cert,
        client_key: args.client_key,
        client_name: "Raspberry Pi client",
    };
    run_native_client(config, SystemTelemetry::new(), &DEVICE_ID_EMBED).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_is_required_and_missing_readings_are_omitted() {
        assert!(Args::try_parse_from(["extrittio-rpi"]).is_err());
        assert!(Args::try_parse_from(["extrittio-rpi", "--contract", "contract.json"]).is_ok());
        let event = SystemTelemetry { previous_cpu: None }.sample();
        assert_eq!(event.stream_key, "system");
        assert!(event.payload.get("cpuUsagePercent").is_none());
        assert!(
            event
                .payload
                .as_object()
                .unwrap()
                .values()
                .all(serde_json::Value::is_number)
        );
        for key in ["humidity", "batteryLevel", "metadata"] {
            assert!(event.payload.get(key).is_none());
        }
    }
}
