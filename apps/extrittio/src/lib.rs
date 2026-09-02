use anyhow::Result;
use clap::Parser;

mod api;
mod args;
mod commands;
mod config;
mod defaults;
mod esp32;
mod models;
mod output;

pub async fn run() -> Result<()> {
    // reqwest is built with `rustls-no-provider`, so install the process-wide
    // provider before any API command can construct an HTTP client.
    extrittio_backend::init::install_crypto_provider();
    let cli = args::Cli::parse();
    commands::run(cli).await
}
