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

use extrittio_backend::app;
use extrittio_backend::config::{AppConfig, DatabaseConfig};
use extrittio_backend::observability;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let observability = observability::init("extrittio-backend", default_log_filter())?;

    let config = AppConfig::from_env()?;
    info!("Starting extrittio-backend on port {}", config.port);
    if config.rpi_mode {
        let database_tuning = match &config.database {
            DatabaseConfig::Postgres { pool_size, .. } => format!("db_pool_size={pool_size}"),
            DatabaseConfig::Turso { database_path, .. } => {
                format!("database_path={}", database_path.display())
            }
        };
        info!(
            "RPI mode enabled ({}, telemetry_retention={}d, log_retention={}d, metrics_interval={}s)",
            database_tuning,
            config.telemetry_retention_days,
            config.log_retention_days,
            config.system_metrics_interval_secs
        );
    }

    let state = app::boot::initialize_state(&config).await?;
    let supervisor = app::workers::spawn_background_tasks(&config, state.clone());
    let server_result = app::http::serve(&config, state, supervisor.cancellation_token()).await;
    let worker_result = supervisor.shutdown().await;
    let observability_result = observability.shutdown();
    server_result?;
    worker_result?;
    observability_result
}

fn default_log_filter() -> &'static str {
    if env_bool("RPI_MODE") {
        "warn,extrittio_backend=warn"
    } else {
        "extrittio_backend=info"
    }
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
