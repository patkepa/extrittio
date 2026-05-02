use anyhow::Result;
use reqwest::Method;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{DeviceTypesCommand, DeviceTypesSubcommand},
    models::{DeviceTypeResponse, Paginated},
    output::{OutputFormat, format_device_types, output},
};

use super::pagination::page_query;

pub(super) async fn handle(
    command: DeviceTypesCommand,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    match command.command {
        DeviceTypesSubcommand::List(args) => {
            let result: Paginated<DeviceTypeResponse> = client
                .get(&format!("/api/v1/device-types{}", page_query(&args)))
                .await?;
            output(output_format, &result, || format_device_types(&result))?;
        }
        DeviceTypesSubcommand::Create { name } => {
            let result: DeviceTypeResponse = client
                .request(
                    Method::POST,
                    "/api/v1/device-types",
                    Some(json!({ "name": name })),
                    true,
                )
                .await?;
            output(output_format, &result, || {
                format!("{} {}", result.id, result.name)
            })?;
        }
        DeviceTypesSubcommand::Delete { id } => {
            client
                .request_empty(Method::DELETE, &format!("/api/v1/device-types/{id}"))
                .await?;
            let value = json!({ "deleted": true, "id": id });
            output(output_format, &value, || {
                format!(
                    "Deleted device type {}",
                    value["id"].as_i64().unwrap_or_default()
                )
            })?;
        }
    }

    Ok(())
}
