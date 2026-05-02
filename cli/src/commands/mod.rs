use anyhow::Result;
use reqwest::Method;
use serde_json::Value;

use crate::{
    api::ApiClient,
    args::{Cli, Command},
    config::{config_path, load_config, normalize_url},
    defaults::DEFAULT_URL,
    output::output,
};

mod api_keys;
mod auth;
mod certs;
mod config_cmd;
mod device_types;
mod devices;
mod fleets;
mod pagination;
mod provision;

pub(crate) async fn run(cli: Cli) -> Result<()> {
    let config_path = config_path(cli.config.as_deref())?;
    let mut config = load_config(&config_path)?;
    let base_url = cli
        .url
        .clone()
        .or_else(|| config.url.clone())
        .unwrap_or_else(|| DEFAULT_URL.to_string());
    let token = cli.token.clone().or_else(|| config.token.clone());
    let client = ApiClient::new(normalize_url(&base_url), token);

    match cli.command {
        Command::Auth(command) => {
            auth::handle(command, cli.output, &client, &config_path, &mut config).await?
        }
        Command::Config(command) => {
            config_cmd::handle(command, cli.output, &client, &config_path, &mut config)?
        }
        Command::Health => {
            let value: Value = client.request(Method::GET, "/health", None, false).await?;
            output(cli.output, &value, || "Backend is healthy".to_string())?;
        }
        Command::Devices(command) => devices::handle(command, cli.output, &client).await?,
        Command::DeviceTypes(command) => device_types::handle(command, cli.output, &client).await?,
        Command::Fleets(command) => fleets::handle(command, cli.output, &client).await?,
        Command::ApiKeys(command) => api_keys::handle(command, cli.output, &client).await?,
        Command::Certs(command) => certs::handle(command, cli.output, &client).await?,
        Command::Provision(args) => provision::handle(args, cli.output, &client).await?,
    }

    Ok(())
}
