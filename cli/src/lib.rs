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
    let cli = args::Cli::parse();
    commands::run(cli).await
}
