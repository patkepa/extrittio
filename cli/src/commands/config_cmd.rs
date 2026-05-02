use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{ConfigCommand, ConfigSubcommand},
    config::{CliConfig, normalize_url, save_config},
    defaults::DEFAULT_URL,
    output::{OutputFormat, output},
};

pub(super) fn handle(
    command: ConfigCommand,
    output_format: OutputFormat,
    client: &ApiClient,
    config_path: &Path,
    config: &mut CliConfig,
) -> Result<()> {
    match command.command {
        ConfigSubcommand::Show => {
            let value = json!({
                "config_path": config_path,
                "url": client.base_url(),
                "token_saved": config.token.is_some(),
                "token_active": client.has_token(),
            });
            output(output_format, &value, || {
                format!(
                    "config: {}\nurl: {}\ntoken_saved: {}\ntoken_active: {}",
                    value["config_path"].as_str().unwrap_or_default(),
                    value["url"].as_str().unwrap_or_default(),
                    value["token_saved"].as_bool().unwrap_or(false),
                    value["token_active"].as_bool().unwrap_or(false),
                )
            })?;
        }
        ConfigSubcommand::SetUrl { url } => {
            config.url = Some(normalize_url(&url));
            save_config(config_path, config)?;
            let value = json!({ "url": config.url.as_deref().unwrap_or(DEFAULT_URL) });
            output(output_format, &value, || {
                format!(
                    "Saved backend URL: {}",
                    config.url.as_deref().unwrap_or(DEFAULT_URL)
                )
            })?;
        }
    }

    Ok(())
}
