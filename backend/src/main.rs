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

    let state = app::boot::initialize_state(&config).await?;
    app::workers::spawn_background_tasks(&config, state.clone());
    app::http::serve(&config, state).await
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("extrittio_backend=info")),
        )
        .init();
}
