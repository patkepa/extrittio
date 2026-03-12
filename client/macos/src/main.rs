mod config;
mod install;
mod metrics;
mod ota;

use clap::{Parser, Subcommand};

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
    // Will be implemented in Task 7
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

    todo!("Runtime implementation in Task 7");
}
