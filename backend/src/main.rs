#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::doc_markdown,
    clippy::similar_names,
    clippy::too_many_lines
)]

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use std::env;

use tracing::info;
use tracing_subscriber::EnvFilter;

use extrittio_backend::app;
use extrittio_backend::config::AppConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let config = AppConfig::from_env();
    info!("Starting extrittio-backend on port {}", config.port);
    if config.rpi_mode {
        info!(
            "RPI mode enabled (db_pool_size={}, telemetry_retention={}d, log_retention={}d, metrics_interval={}s)",
            config.db_pool_size,
            config.telemetry_retention_days,
            config.log_retention_days,
            config.system_metrics_interval_secs
        );
    }

    let state = app::boot::initialize_state(&config).await?;
    app::workers::spawn_background_tasks(&config, state.clone());
    app::http::serve(&config, state).await
}

fn init_tracing() {
    let default_filter = if env_bool("RPI_MODE") {
        "warn,extrittio_backend=warn"
    } else {
        "extrittio_backend=info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .init();
}

fn env_bool(key: &str) -> bool {
    env::var(key)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}
