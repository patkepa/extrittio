use anyhow::{Result, anyhow, bail};
use reqwest::Method;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{CreateDeviceArgs, DevicesCommand, DevicesSubcommand},
    models::{DeviceResponse, DeviceTypeResponse, Paginated},
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
    let device_type_id =
        resolve_device_type_id(client, args.device_type_id, args.device_type.as_deref()).await?;
    client
        .request(
            Method::POST,
            "/api/v1/devices",
            Some(json!({
                "name": args.name,
                "device_type_id": device_type_id,
                "blueprint_revision_id": args.blueprint_revision_id,
                "fleet_id": args.fleet_id,
                "firmware": args.firmware,
            })),
            true,
        )
        .await
}

async fn resolve_device_type_id(
    client: &ApiClient,
    device_type_id: Option<i32>,
    device_type_name: Option<&str>,
) -> Result<i32> {
    if device_type_id.is_some() && device_type_name.is_some() {
        bail!("pass either --device-type-id or --device-type, not both");
    }
    if let Some(device_type_id) = device_type_id {
        return Ok(device_type_id);
    }

    let device_type_name = device_type_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("pass --device-type-id or --device-type"))?;
    let existing: Paginated<DeviceTypeResponse> = client
        .get("/api/v1/device-types?limit=1000&offset=0")
        .await?;
    if let Some(device_type) = existing
        .data
        .iter()
        .find(|device_type| device_type.name.eq_ignore_ascii_case(device_type_name))
    {
        return Ok(device_type.id);
    }

    let created: DeviceTypeResponse = client
        .request(
            Method::POST,
            "/api/v1/device-types",
            Some(json!({ "name": device_type_name })),
            true,
        )
        .await?;
    Ok(created.id)
}
