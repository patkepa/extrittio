use std::fmt::Write as _;

use crate::models::{ApiKeyResponse, DeviceResponse, DeviceTypeResponse, FleetResponse, Paginated};
use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::Serialize;

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
