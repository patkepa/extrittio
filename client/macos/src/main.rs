mod config;
mod install;
mod location;
mod metrics;
mod ota;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use extrittio_common::extrittio::{
    DeviceHeartbeat, DeviceTelemetry, ShadowDelta, ShadowGet,
};
use extrittio_common::{device_status, ota::fields as ota_fields, topics};
use prost::Message;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "extrittio-macos", about = "macOS system telemetry client for Extrittio IoT Hub")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the client (normal operation)
    Run {
        /// Override Zenoh endpoint from config
        #[arg(long)]
        connect: Option<String>,

        /// Override telemetry interval (seconds)
        #[arg(long)]
        interval: Option<u64>,

        /// Override heartbeat interval (seconds)
        #[arg(long)]
        heartbeat_interval: Option<u64>,
    },
    /// Create config + install LaunchAgent
    Install {
        /// Device ID to write into default config
        #[arg(long)]
        device_id: Option<String>,

        /// Zenoh endpoint to write into default config
        #[arg(long)]
        connect: Option<String>,
    },
    /// Remove LaunchAgent (keeps config)
    Uninstall,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            connect,
            interval,
            heartbeat_interval,
        } => {
            run(connect, interval, heartbeat_interval);
        }
        Commands::Install { device_id, connect } => {
            install::install(device_id.as_deref(), connect.as_deref());
        }
        Commands::Uninstall => {
            install::uninstall();
        }
    }
}

fn run(
    connect_override: Option<String>,
    interval_override: Option<u64>,
    heartbeat_override: Option<u64>,
) {
    // Load config, apply overrides
    let mut cfg = config::Config::load();
    if let Some(c) = connect_override {
        cfg.connect = Some(c);
    }
    if let Some(i) = interval_override {
        cfg.telemetry_interval_secs = i;
    }
    if let Some(h) = heartbeat_override {
        cfg.heartbeat_interval_secs = h;
    }

    let device_id = cfg.device_id.clone().unwrap_or_else(|| {
        eprintln!(
            "Error: no device_id set.\n\
             Run: extrittio-macos install --device-id <ID>\n\
             Or add device_id to {}", config::Config::config_path().display()
        );
        std::process::exit(1);
    });

    // Start the async runtime
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(run_async(cfg, device_id));
}

async fn run_async(cfg: config::Config, device_id: String) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "extrittio_macos=info".into()),
        )
        .init();

    tracing::info!(
        "Starting macOS client '{}' (telemetry every {}s, heartbeat every {}s)",
        device_id,
        cfg.telemetry_interval_secs,
        cfg.heartbeat_interval_secs
    );

    // -- Zenoh session setup --
    let mut zenoh_config = zenoh::Config::default();

    if let Some(ref endpoint) = cfg.connect {
        zenoh_config
            .insert_json5("connect/endpoints", &format!("[\"{endpoint}\"]"))
            .expect("Failed to set Zenoh connect endpoint");
        zenoh_config
            .insert_json5("scouting/multicast/enabled", "false")
            .expect("Failed to disable multicast scouting");
    }

    if let Some(ref tls) = cfg.tls {
        if let Some(ref ca_cert) = tls.ca_cert {
            let ca_path = std::fs::canonicalize(ca_cert)
                .unwrap_or_else(|e| panic!("CA cert not found at '{ca_cert}': {e}"));
            zenoh_config
                .insert_json5(
                    "transport/link/tls/root_ca_certificate",
                    &format!("\"{}\"", ca_path.display()),
                )
                .expect("Failed to set TLS root CA");
        }

        if let Some(ref client_cert) = tls.client_cert {
            let cert_path = std::fs::canonicalize(client_cert)
                .unwrap_or_else(|e| panic!("Client cert not found at '{client_cert}': {e}"));
            zenoh_config
                .insert_json5(
                    "transport/link/tls/connect_certificate",
                    &format!("\"{}\"", cert_path.display()),
                )
                .expect("Failed to set TLS client certificate");
        }

        if let Some(ref client_key) = tls.client_key {
            let key_path = std::fs::canonicalize(client_key)
                .unwrap_or_else(|e| panic!("Client key not found at '{client_key}': {e}"));
            zenoh_config
                .insert_json5(
                    "transport/link/tls/connect_private_key",
                    &format!("\"{}\"", key_path.display()),
                )
                .expect("Failed to set TLS client private key");
        }
    }

    let session = zenoh::open(zenoh_config)
        .await
        .expect("Failed to open zenoh session");
    let session = Arc::new(session);

    tracing::info!("Zenoh session opened");

    let telemetry_topic = topics::telemetry(&device_id);
    let heartbeat_topic = topics::heartbeat(&device_id);
    let shadow_get_topic = topics::shadow_get(&device_id);
    let shadow_delta_topic = topics::shadow_delta(&device_id);
    let shadow_report_topic = topics::shadow_report(&device_id);

    let reported_state: Arc<Mutex<serde_json::Map<String, serde_json::Value>>> =
        Arc::new(Mutex::new(serde_json::Map::new()));
    let firmware_version: Arc<Mutex<String>> =
        Arc::new(Mutex::new(cfg.firmware_version.clone()));
    let ota_in_progress: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    let start = Instant::now();

    // -- Heartbeat task --
    let hb_session = session.clone();
    let hb_device_id = device_id.clone();
    let hb_interval = cfg.heartbeat_interval_secs;
    let hb_firmware = firmware_version.clone();
    let hb_heartbeat_topic = heartbeat_topic.clone();
    let hb_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(hb_interval));
        loop {
            interval.tick().await;

            let fw = hb_firmware.lock().await.clone();
            let heartbeat = DeviceHeartbeat {
                device_id: hb_device_id.clone(),
                timestamp: extrittio_sdk::time::now_millis(),
                status: device_status::ONLINE.to_string(),
                firmware: fw,
                #[allow(clippy::cast_possible_wrap)]
                uptime_seconds: start.elapsed().as_secs() as i64,
            };

            let payload = heartbeat.encode_to_vec();
            if let Err(e) = hb_session.put(&hb_heartbeat_topic, payload).await {
                tracing::warn!("Failed to send heartbeat: {}", e);
            } else {
                tracing::info!("Heartbeat sent (uptime: {}s)", start.elapsed().as_secs());
            }
        }
    });

    // -- Request pending shadow delta on startup --
    let shadow_get = ShadowGet {
        device_id: device_id.clone(),
    };
    let payload = shadow_get.encode_to_vec();
    if let Err(e) = session.put(&shadow_get_topic, payload).await {
        tracing::warn!("Failed to send ShadowGet: {}", e);
    } else {
        tracing::info!("Sent ShadowGet to '{}'", shadow_get_topic);
    }

    // -- Shadow subscriber task --
    let shadow_session = session.clone();
    let shadow_device_id = device_id.clone();
    let shadow_reported = reported_state.clone();
    let shadow_firmware = firmware_version.clone();
    let shadow_ota_flag = ota_in_progress.clone();
    let shadow_task = tokio::spawn(async move {
        let subscriber = shadow_session
            .declare_subscriber(&shadow_delta_topic)
            .await
            .expect("Failed to subscribe to shadow/delta");

        tracing::info!("Subscribed to '{}'", shadow_delta_topic);

        loop {
            let sample = subscriber.recv_async().await;
            match sample {
                Ok(sample) => {
                    let bytes = sample.payload().to_bytes();
                    match ShadowDelta::decode(bytes.as_ref()) {
                        Ok(delta) => {
                            tracing::info!("Shadow delta received: {}", delta.delta_json);

                            match serde_json::from_str::<serde_json::Value>(&delta.delta_json) {
                                Ok(serde_json::Value::Object(mut delta_map)) => {
                                    let ota_payload = delta_map.remove(ota_fields::SHADOW_KEY);

                                    {
                                        let mut state = shadow_reported.lock().await;
                                        for (key, value) in delta_map {
                                            state.insert(key, value);
                                        }
                                    }

                                    ota::send_shadow_report(
                                        &shadow_device_id,
                                        &shadow_session,
                                        &shadow_report_topic,
                                        &shadow_reported,
                                        delta.version,
                                    )
                                    .await;

                                    if let Some(ota_val) = ota_payload {
                                        if shadow_ota_flag
                                            .compare_exchange(
                                                false, true,
                                                Ordering::SeqCst, Ordering::SeqCst,
                                            )
                                            .is_err()
                                        {
                                            tracing::warn!(
                                                "OTA: update already in progress, ignoring"
                                            );
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
                                            ota::handle_ota(
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
                                    tracing::warn!("Shadow delta JSON is not an object");
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to parse shadow delta JSON: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to decode ShadowDelta: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Shadow subscriber error: {}, retrying in 5s...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            }
        }
    });

    // -- Telemetry loop (main task) — real system metrics --
    let mut collector = metrics::MetricsCollector::new();
    let telemetry_interval = cfg.telemetry_interval_secs;
    let mut interval = tokio::time::interval(Duration::from_secs(telemetry_interval));

    tracing::info!("Sending system telemetry to '{}'", telemetry_topic);

    // Graceful shutdown: listen for SIGTERM/SIGINT
    let shutdown = async {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to register SIGTERM handler");
        let sigint = tokio::signal::ctrl_c();
        tokio::select! {
            _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
            _ = sigint => tracing::info!("Received SIGINT"),
        }
    };
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                collector.refresh();

                let battery_level = collector.battery_level();
                let metadata = collector.extended_metrics();

                let telemetry = DeviceTelemetry {
                    device_id: device_id.clone(),
                    timestamp: extrittio_sdk::time::now_millis(),
                    temperature: 0.0,
                    humidity: 0.0,
                    battery_level,
                    metadata,
                    latitude: 0.0,
                    longitude: 0.0,
                    speed: 0.0,
                    altitude: 0.0,
                    heading: 0.0,
                };

                let payload = telemetry.encode_to_vec();
                if let Err(e) = session.put(&telemetry_topic, payload).await {
                    tracing::warn!("Failed to send telemetry: {}", e);
                } else {
                    tracing::info!(
                        "Telemetry: battery={:.1}% cpu={}% mem={}% load={}",
                        battery_level,
                        telemetry.metadata.get("cpu_usage_percent").map_or("-", String::as_str),
                        telemetry.metadata.get("memory_usage_percent").map_or("-", String::as_str),
                        telemetry.metadata.get("load_1m").map_or("-", String::as_str),
                    );
                }
            }
            _ = &mut shutdown => {
                tracing::info!("Shutting down...");
                break;
            }
        }
    }

    // Cancel background tasks
    hb_task.abort();
    shadow_task.abort();

    // Send final OFFLINE heartbeat
    let fw = firmware_version.lock().await.clone();
    let offline_heartbeat = DeviceHeartbeat {
        device_id: device_id.clone(),
        timestamp: extrittio_sdk::time::now_millis(),
        status: device_status::OFFLINE.to_string(),
        firmware: fw,
        #[allow(clippy::cast_possible_wrap)]
        uptime_seconds: start.elapsed().as_secs() as i64,
    };
    let payload = offline_heartbeat.encode_to_vec();
    if let Err(e) = session.put(&heartbeat_topic, payload).await {
        tracing::warn!("Failed to send OFFLINE heartbeat: {}", e);
    } else {
        tracing::info!("OFFLINE heartbeat sent. Goodbye.");
    }
}
