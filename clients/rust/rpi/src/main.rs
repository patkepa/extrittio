mod metrics;

use std::time::Duration;

use clap::Parser;
use extrittio_client_runtime::{
    EMBED_MARKER_LEN, EMBED_SLOT_LEN, NativeClientConfig, TelemetrySource, contract::ContractEvent,
    run_native_client,
};
use extrittio_common::extrittio::DeviceTelemetry;

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
    contract: Option<String>,
    #[arg(long, default_value_t = 10)]
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

impl TelemetrySource for SystemTelemetry {
    fn sample(&mut self, device_id: &str, timestamp: i64) -> DeviceTelemetry {
        let cpu_usage = match (metrics::CpuSnapshot::take(), &self.previous_cpu) {
            (Some(current), Some(previous)) => {
                let usage = current.usage_since(previous);
                self.previous_cpu = Some(current);
                usage
            }
            (Some(current), None) => {
                self.previous_cpu = Some(current);
                0.0
            }
            _ => 0.0,
        };
        let cpu_temperature = metrics::cpu_temperature();
        let memory_usage = metrics::memory_usage_percent();
        let mut metadata = metrics::extended_metrics();
        metadata.insert("cpu_usage_pct".into(), format!("{cpu_usage:.1}"));
        metadata.insert("mem_used_pct".into(), format!("{memory_usage:.1}"));

        DeviceTelemetry {
            device_id: device_id.to_string(),
            timestamp,
            temperature: cpu_temperature,
            humidity: 0.0,
            battery_level: 0.0,
            metadata,
            latitude: 0.0,
            longitude: 0.0,
            speed: 0.0,
            altitude: 0.0,
            heading: 0.0,
        }
    }

    fn summary(&self, telemetry: &DeviceTelemetry) -> String {
        format!(
            "Telemetry: cpu_temp={:.1}°C mem={}% cpu={}% load={}",
            telemetry.temperature,
            telemetry
                .metadata
                .get("mem_used_pct")
                .map_or("-", String::as_str),
            telemetry
                .metadata
                .get("cpu_usage_pct")
                .map_or("-", String::as_str),
            telemetry
                .metadata
                .get("load_1m")
                .map_or("-", String::as_str),
        )
    }

    fn contract_event(&self, telemetry: &DeviceTelemetry) -> Option<ContractEvent> {
        let number = |key: &str| {
            telemetry
                .metadata
                .get(key)
                .and_then(|value| value.parse::<f64>().ok())
        };
        Some(ContractEvent::new(
            "system",
            serde_json::json!({
                "cpuTemperature": telemetry.temperature,
                "cpuUsagePercent": number("cpu_usage_pct"),
                "memoryUsagePercent": number("mem_used_pct"),
                "load1m": number("load_1m"),
                "load5m": number("load_5m"),
                "load15m": number("load_15m")
            }),
        ))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "extrittio_rpi=info".into()),
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
        client_name: "Raspberry Pi client",
    };
    run_native_client(config, SystemTelemetry::new(), &DEVICE_ID_EMBED).await;
}
