use std::{
    io::{self, Write},
    path::Path,
};

use anyhow::{Context, Result};
use reqwest::Method;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{AuthCommand, AuthSubcommand},
    config::{CliConfig, save_config},
    models::{LoginResponse, UserResponse},
    output::{OutputFormat, output},
};

pub(super) async fn handle(
    command: AuthCommand,
    output_format: OutputFormat,
    client: &ApiClient,
    config_path: &Path,
    config: &mut CliConfig,
) -> Result<()> {
    match command.command {
        AuthSubcommand::Login(args) => {
            let password = match args.password {
                Some(password) => password,
                None => prompt_password()?,
            };
            let response: LoginResponse = client
                .request(
                    Method::POST,
                    "/api/v1/auth/login",
                    Some(json!({
                        "username": args.username,
                        "password": password,
                        "issue_token": true,
                    })),
                    false,
                )
                .await?;

            if !args.no_save {
                config.url = Some(client.base_url().to_string());
                config.token = Some(response.token.clone());
                save_config(config_path, config)?;
            }

            output(output_format, &response, || {
                format!(
                    "Logged in as {} ({})",
                    response.user.username, response.user.role
                )
            })?;
        }
        AuthSubcommand::Me => {
            let user: UserResponse = client.get("/api/v1/auth/me").await?;
            output(output_format, &user, || {
                format!("{} ({}) id={}", user.username, user.role, user.id)
            })?;
        }
        AuthSubcommand::Logout => {
            config.token = None;
            save_config(config_path, config)?;
            let value = json!({ "logged_out": true });
            output(output_format, &value, || "Logged out".to_string())?;
        }
    }

    Ok(())
}

fn prompt_password() -> Result<String> {
    print!("Password: ");
    io::stdout().flush().context("failed to flush stdout")?;
    let mut password = String::new();
    io::stdin()
        .read_line(&mut password)
        .context("failed to read password")?;
    Ok(password.trim_end_matches(['\r', '\n']).to_string())
}
