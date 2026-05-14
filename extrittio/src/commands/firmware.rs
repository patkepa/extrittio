use std::{fmt::Write as _, fs};

use anyhow::{Context, Result};
use reqwest::{Method, multipart};
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{FirmwareCommand, FirmwareSubcommand, ListFirmwareArgs, UploadFirmwareArgs},
    models::{FirmwareUpdateResponse, Paginated},
    output::{OutputFormat, output},
};

pub(super) async fn handle(
    command: FirmwareCommand,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    match command.command {
        FirmwareSubcommand::List(args) => {
            let query = firmware_list_query(&args);
            let firmware: Paginated<FirmwareUpdateResponse> = client
                .get(&format!("/api/v1/firmware-updates{query}"))
                .await?;
            output(output_format, &firmware, || format_firmware_list(&firmware))?;
        }
        FirmwareSubcommand::Upload(args) => {
            let firmware = upload_firmware(client, args).await?;
            output(output_format, &firmware, || format_firmware(&firmware))?;
        }
        FirmwareSubcommand::Delete { id } => {
            client
                .request_empty(Method::DELETE, &format!("/api/v1/firmware-updates/{id}"))
                .await?;
            let value = json!({ "deleted": true, "id": id });
            output(output_format, &value, || format!("Deleted firmware {id}"))?;
        }
    }

    Ok(())
}

async fn upload_firmware(
    client: &ApiClient,
    args: UploadFirmwareArgs,
) -> Result<FirmwareUpdateResponse> {
    let filename = args
        .file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("firmware.bin")
        .to_string();
    let data = fs::read(&args.file)
        .with_context(|| format!("failed to read firmware file {}", args.file.display()))?;

    let file_part = multipart::Part::bytes(data)
        .file_name(filename)
        .mime_str("application/octet-stream")
        .context("failed to build firmware file part")?;
    let mut form = multipart::Form::new()
        .text("device_type_id", args.device_type_id.to_string())
        .part("file", file_part);

    if let Some(version) = args.version {
        form = form.text("version", version);
    }
    if let Some(description) = args.description {
        form = form.text("description", description);
    }

    client
        .request_multipart("/api/v1/firmware-updates/upload", form)
        .await
}

fn firmware_list_query(args: &ListFirmwareArgs) -> String {
    let mut params = vec![
        format!("limit={}", args.limit),
        format!("offset={}", args.offset),
    ];
    if let Some(device_type_id) = args.device_type_id {
        params.push(format!("device_type_id={device_type_id}"));
    }
    format!("?{}", params.join("&"))
}

fn format_firmware_list(firmware: &Paginated<FirmwareUpdateResponse>) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<8} {:<22} {:<28} {:<9} {:<10} FILE",
        "ID", "VERSION", "DEVICE_TYPE", "BLOB", "SIZE"
    );
    for fw in &firmware.data {
        let _ = writeln!(
            out,
            "{:<8} {:<22} {:<28} {:<9} {:<10} {}",
            fw.id,
            truncate(&fw.version, 22),
            truncate(&fw.device_type_name, 28),
            if fw.has_blob { "yes" } else { "no" },
            fw.file_size
                .map(|size| size.to_string())
                .unwrap_or_else(|| "-".to_string()),
            fw.filename.as_deref().unwrap_or("-")
        );
    }
    let _ = write!(
        out,
        "\nshowing {} of {} (limit={}, offset={})",
        firmware.data.len(),
        firmware.total,
        firmware.limit,
        firmware.offset
    );
    out
}

fn format_firmware(fw: &FirmwareUpdateResponse) -> String {
    format!(
        "id={}\ndevice_type={} ({})\nversion={}\nurl={}\nsha256={}\nfile={}\nsize={}",
        fw.id,
        fw.device_type_name,
        fw.device_type_id,
        fw.version,
        fw.url,
        fw.sha256.as_deref().unwrap_or("-"),
        fw.filename.as_deref().unwrap_or("-"),
        fw.file_size
            .map(|size| size.to_string())
            .unwrap_or_else(|| "-".to_string()),
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut result = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    result.push_str("...");
    result
}
