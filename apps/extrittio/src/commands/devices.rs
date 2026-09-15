use anyhow::Result;
use reqwest::Method;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{CreateDeviceArgs, DevicesCommand, DevicesSubcommand},
    models::{DeviceResponse, Paginated},
    output::{OutputFormat, format_device, format_devices, output},
};

use super::pagination::device_list_query;

pub(super) async fn handle(
    command: DevicesCommand,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    match command.command {
        DevicesSubcommand::List(args) => {
            let query = device_list_query(&args);
            let devices: Paginated<DeviceResponse> =
                client.get(&format!("/api/v1/devices{query}")).await?;
            output(output_format, &devices, || format_devices(&devices))?;
        }
        DevicesSubcommand::Get { id } => {
            let device: DeviceResponse = client.get(&format!("/api/v1/devices/{id}")).await?;
            output(output_format, &device, || format_device(&device))?;
        }
        DevicesSubcommand::Create(args) => {
            let device = create_device(client, args).await?;
            output(output_format, &device, || format_device(&device))?;
        }
        DevicesSubcommand::Delete { id } => {
            client
                .request_empty(Method::DELETE, &format!("/api/v1/devices/{id}"))
                .await?;
            let value = json!({ "deleted": true, "id": id });
            output(output_format, &value, || {
                format!(
                    "Deleted device {}",
                    value["id"].as_str().unwrap_or_default()
                )
            })?;
        }
    }

    Ok(())
}

pub(super) async fn create_device(
    client: &ApiClient,
    args: CreateDeviceArgs,
) -> Result<DeviceResponse> {
    client
        .request(
            Method::POST,
            "/api/v1/devices",
            Some(json!({
                "name": args.name,
                "blueprint_revision_id": args.blueprint_revision_id,
                "fleet_id": args.fleet_id,
                "firmware": args.firmware,
            })),
            true,
        )
        .await
}
