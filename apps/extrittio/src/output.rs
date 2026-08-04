use std::fmt::Write as _;

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::Serialize;
use serde_json::Value;

use crate::{
    defaults::{DEFAULT_ESP32_NVS_OFFSET, DEFAULT_ESP32_NVS_SIZE, DEFAULT_ZENOH_CONNECT},
    models::{ApiKeyResponse, DeviceResponse, DeviceTypeResponse, FleetResponse, Paginated},
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
    Table,
    Json,
}

pub(crate) fn output<T: Serialize, F: FnOnce() -> String>(
    output_format: OutputFormat,
    value: &T,
    table: F,
) -> Result<()> {
    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).context("failed to serialize JSON output")?
            );
        }
        OutputFormat::Table => println!("{}", table()),
    }
    Ok(())
}

pub(crate) fn format_devices(devices: &Paginated<DeviceResponse>) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<38} {:<24} {:<14} {:<12} {:<16} FIRMWARE",
        "ID", "NAME", "TYPE", "STATUS", "FLEET"
    );
    for device in &devices.data {
        let _ = writeln!(
            out,
            "{:<38} {:<24} {:<14} {:<12} {:<16} {}",
            truncate(&device.id, 38),
            truncate(&device.name, 24),
            truncate(&device.device_type_name, 14),
            truncate(&device.status, 12),
            truncate(device.fleet_name.as_deref().unwrap_or("-"), 16),
            device.firmware
        );
    }
    let _ = write!(
        out,
        "\nshowing {} of {} (limit={}, offset={})",
        devices.data.len(),
        devices.total,
        devices.limit,
        devices.offset
    );
    out
}

pub(crate) fn format_device(device: &DeviceResponse) -> String {
    format!(
        "id={}\nname={}\ntype={} ({})\nfleet={}\nstatus={}\nlast_seen={}\nfirmware={}\nuptime={}",
        device.id,
        device.name,
        device.device_type_name,
        device.device_type_id,
        device.fleet_name.as_deref().unwrap_or("-"),
        device.status,
        device.last_seen,
        device.firmware,
        device.uptime,
    )
}

pub(crate) fn format_device_types(device_types: &Paginated<DeviceTypeResponse>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:<8} NAME", "ID");
    for device_type in &device_types.data {
        let _ = writeln!(out, "{:<8} {}", device_type.id, device_type.name);
    }
    let _ = write!(
        out,
        "\nshowing {} of {}",
        device_types.data.len(),
        device_types.total
    );
    out
}

pub(crate) fn format_fleets(fleets: &Paginated<FleetResponse>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:<8} {:<28} DEVICES", "ID", "NAME");
    for fleet in &fleets.data {
        let _ = writeln!(
            out,
            "{:<8} {:<28} {}",
            fleet.id,
            truncate(&fleet.name, 28),
            fleet.device_count
        );
    }
    let _ = write!(out, "\nshowing {} of {}", fleets.data.len(), fleets.total);
    out
}

pub(crate) fn format_api_keys(api_keys: &[ApiKeyResponse]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<8} {:<24} {:<16} {:<18} LAST_USED",
        "ID", "NAME", "PREFIX", "DEVICE_TYPE"
    );
    for key in api_keys {
        let _ = writeln!(
            out,
            "{:<8} {:<24} {:<16} {:<18} {}",
            key.id,
            truncate(&key.name, 24),
            key.key_prefix,
            truncate(key.device_type_name.as_deref().unwrap_or("-"), 18),
            key.last_used_at.as_deref().unwrap_or("-")
        );
    }
    out
}

pub(crate) fn format_provisioning(value: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "device_id={}",
        value["device_id"].as_str().unwrap_or_default()
    );
    let _ = writeln!(out, "name={}", value["name"].as_str().unwrap_or_default());
    let _ = writeln!(
        out,
        "device_type={} ({})",
        value["device_type_name"].as_str().unwrap_or_default(),
        value["device_type_id"].as_i64().unwrap_or_default(),
    );
    if let Some(fleet_name) = value["fleet_name"].as_str() {
        let _ = writeln!(out, "fleet={fleet_name}");
    }
    let _ = writeln!(
        out,
        "firmware={}",
        value["firmware"].as_str().unwrap_or_default()
    );
    let _ = writeln!(
        out,
        "zenoh_connect={}",
        value["zenoh_connect"]
            .as_str()
            .unwrap_or(DEFAULT_ZENOH_CONNECT)
    );
    if let Some(cert_dir) = value["certificate_dir"].as_str() {
        let _ = writeln!(out, "certificate_dir={cert_dir}");
    }
    if let Some(esp32_nvs) = value["esp32_nvs"].as_object() {
        let _ = writeln!(
            out,
            "esp32_nvs=flashed port={} offset={} size={}",
            esp32_nvs
                .get("port")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            esp32_nvs
                .get("nvs_offset")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_ESP32_NVS_OFFSET),
            esp32_nvs
                .get("nvs_size")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_ESP32_NVS_SIZE),
        );
        if let Some(csv_path) = esp32_nvs.get("csv_path").and_then(Value::as_str) {
            let _ = writeln!(out, "nvs_csv={csv_path}");
        }
        if let Some(bin_path) = esp32_nvs.get("bin_path").and_then(Value::as_str) {
            let _ = writeln!(out, "nvs_bin={bin_path}");
        }
    }
    out
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
