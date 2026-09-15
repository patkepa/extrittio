use std::path::PathBuf;

use anyhow::Result;
use reqwest::Method;
use serde_json::Value;

use crate::{
    api::ApiClient,
    args::{Cli, Command},
    config::{config_path, load_config, normalize_url},
    defaults::DEFAULT_URL,
    output::{OutputFormat, output},
};

mod api_keys;
mod auth;
mod certs;
mod config_cmd;
mod devices;
mod firmware;
mod fleets;
mod ota;
mod pagination;
mod provision;
mod service;

pub(crate) async fn run(cli: Cli) -> Result<()> {
    let command = cli
        .command
        .unwrap_or_else(|| Command::Serve(Default::default()));
    let output_format = cli.output;

    match command {
        Command::Run(args) => service::run_edge(args).await,
        Command::Serve(args) => service::serve(args).await,
        Command::Migrate(args) => service::migrate(args, output_format).await,
        Command::Init(args) => service::init(args, output_format).await,
        Command::Database(args) => service::database(args, output_format).await,
        command => run_api_command(command, cli.url, cli.token, cli.config, output_format).await,
    }
}

async fn run_api_command(
    command: Command,
    url: Option<String>,
    token: Option<String>,
    config_file: Option<PathBuf>,
    output_format: OutputFormat,
) -> Result<()> {
    let config_path = config_path(config_file.as_deref())?;
    let mut config = load_config(&config_path)?;
    let base_url = url
        .or_else(|| config.url.clone())
        .unwrap_or_else(|| DEFAULT_URL.to_string());
    let token = token.or_else(|| config.token.clone());
    let client = ApiClient::new(normalize_url(&base_url), token);

    match command {
        Command::Auth(command) => {
            auth::handle(command, output_format, &client, &config_path, &mut config).await?
        }
        Command::Config(command) => {
            config_cmd::handle(command, output_format, &client, &config_path, &mut config)?
        }
        Command::Health => {
            let value: Value = client.request(Method::GET, "/health", None, false).await?;
            output(output_format, &value, || "Backend is healthy".to_string())?;
        }
        Command::Ready => {
            let value: Value = client.request(Method::GET, "/ready", None, false).await?;
            output(output_format, &value, || "Backend is ready".to_string())?;
        }
        Command::Devices(command) => devices::handle(command, output_format, &client).await?,
        Command::Fleets(command) => fleets::handle(command, output_format, &client).await?,
        Command::Firmware(command) => firmware::handle(command, output_format, &client).await?,
        Command::Ota(command) => ota::handle(command, output_format, &client).await?,
        Command::ApiKeys(command) => api_keys::handle(command, output_format, &client).await?,
        Command::Certs(command) => certs::handle(command, output_format, &client).await?,
        Command::Provision(args) => provision::handle(args, output_format, &client).await?,
        Command::Run(_)
        | Command::Serve(_)
        | Command::Migrate(_)
        | Command::Init(_)
        | Command::Database(_) => {
            unreachable!()
        }
    }

    Ok(())
}
