use anyhow::Result;
use reqwest::Method;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{ApiKeysCommand, ApiKeysSubcommand},
    models::{ApiKeyResponse, CreatedApiKeyResponse},
    output::{OutputFormat, format_api_keys, output},
};

pub(super) async fn handle(
    command: ApiKeysCommand,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    match command.command {
        ApiKeysSubcommand::List => {
            let result: Vec<ApiKeyResponse> = client.get("/api/v1/api-keys").await?;
            output(output_format, &result, || format_api_keys(&result))?;
        }
        ApiKeysSubcommand::Create {
            name,
            device_type_id,
        } => {
            let result: CreatedApiKeyResponse = client
                .request(
                    Method::POST,
                    "/api/v1/api-keys",
                    Some(json!({ "name": name, "device_type_id": device_type_id })),
                    true,
                )
                .await?;
            output(output_format, &result, || {
                format!(
                    "id={}\nname={}\nkey={}\nkey_prefix={}",
                    result.id, result.name, result.key, result.key_prefix
                )
            })?;
        }
        ApiKeysSubcommand::Delete { id } => {
            client
                .request_empty(Method::DELETE, &format!("/api/v1/api-keys/{id}"))
                .await?;
            let value = json!({ "deleted": true, "id": id });
            output(output_format, &value, || {
                format!(
                    "Deleted API key {}",
                    value["id"].as_i64().unwrap_or_default()
                )
            })?;
        }
    }

    Ok(())
}
