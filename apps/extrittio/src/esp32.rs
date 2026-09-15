use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    args::ProvisionArgs,
    models::{DeviceResponse, Esp32NvsFlashResult},
};

pub(crate) fn flash_esp32_nvs(
    device: &DeviceResponse,
    args: &ProvisionArgs,
    zenoh_endpoint: &str,
) -> Result<Esp32NvsFlashResult> {
    let wifi_ssid = args
        .wifi_ssid
        .as_deref()
        .filter(|ssid| !ssid.is_empty())
        .ok_or_else(|| {
            anyhow!("--wifi-ssid or EXTRITTIO_WIFI_SSID is required with --flash-esp32-nvs")
        })?;
    let wifi_password = args.wifi_password.as_deref().unwrap_or("");
    let firmware_version = args
        .esp32_firmware_version
        .as_deref()
        .unwrap_or(device.firmware.as_str());
    let port = args
        .port
        .clone()
        .map(Ok)
        .unwrap_or_else(detect_esp_serial_port)?;
    let nvs_gen = resolve_esp_tool(
        args.nvs_partition_gen.as_deref(),
        args.idf_path.as_deref(),
        &[
            "components",
            "nvs_flash",
            "nvs_partition_generator",
            "nvs_partition_gen.py",
        ],
        "nvs_partition_gen.py",
    );
    let esptool = resolve_esp_tool(
        args.esptool.as_deref(),
        args.idf_path.as_deref(),
        &["components", "esptool_py", "esptool", "esptool.py"],
        "esptool.py",
    );
    let idf_python = resolve_idf_python(args.idf_python.as_deref());
    let (csv_path, bin_path) = nvs_artifact_paths(&device.id)?;

    write_esp32_nvs_csv(
        &csv_path,
        &[
            ("device_id", device.id.as_str()),
            ("wifi_ssid", wifi_ssid),
            ("wifi_pass", wifi_password),
            ("zenoh", zenoh_endpoint),
            ("fw_version", firmware_version),
        ],
    )?;

    run_process(
        ProcessCommand::new(&idf_python)
            .arg(&nvs_gen)
            .arg("generate")
            .arg(&csv_path)
            .arg(&bin_path)
            .arg(&args.nvs_size),
        "failed to generate ESP32 NVS image",
    )?;

    run_process(
        ProcessCommand::new(&idf_python)
            .arg(&esptool)
            .arg("--chip")
            .arg(&args.chip)
            .arg("-p")
            .arg(&port)
            .arg("-b")
            .arg(args.baud.to_string())
            .arg("--before")
            .arg("default_reset")
            .arg("--after")
            .arg("hard_reset")
            .arg("write_flash")
            .arg(&args.nvs_offset)
            .arg(&bin_path),
        "failed to flash ESP32 NVS image",
    )?;

    let result = Esp32NvsFlashResult {
        port: port.display().to_string(),
        chip: args.chip.clone(),
        baud: args.baud,
        nvs_offset: args.nvs_offset.clone(),
        nvs_size: args.nvs_size.clone(),
        csv_path: args.keep_nvs_artifacts.then(|| csv_path.clone()),
        bin_path: args.keep_nvs_artifacts.then(|| bin_path.clone()),
    };

    if !args.keep_nvs_artifacts {
        let _ = fs::remove_file(&csv_path);
        let _ = fs::remove_file(&bin_path);
    }

    Ok(result)
}

fn resolve_esp_tool(
    explicit: Option<&Path>,
    idf_path: Option<&Path>,
    idf_relative: &[&str],
    fallback: &str,
) -> PathBuf {
    if let Some(explicit) = explicit {
        return explicit.to_path_buf();
    }
    if let Some(idf_path) = idf_path {
        let mut path = idf_path.to_path_buf();
        for segment in idf_relative {
            path.push(segment);
        }
        return path;
    }
    PathBuf::from(fallback)
}

fn resolve_idf_python(explicit: Option<&Path>) -> PathBuf {
    if let Some(explicit) = explicit {
        return explicit.to_path_buf();
    }
    if let Ok(env_path) = env::var("IDF_PYTHON_ENV_PATH") {
        return PathBuf::from(env_path).join("bin").join("python");
    }
    PathBuf::from("python")
}

fn nvs_artifact_paths(device_id: &str) -> Result<(PathBuf, PathBuf)> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_millis();
    let safe_id = device_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let base = env::temp_dir().join(format!(
        "extrittio-{safe_id}-{}-{nonce}",
        std::process::id()
    ));
    Ok((base.with_extension("csv"), base.with_extension("bin")))
}

fn write_esp32_nvs_csv(path: &Path, entries: &[(&str, &str)]) -> Result<()> {
    let mut csv = String::from("key,type,encoding,value\nextrittio,namespace,,\n");
    for (key, value) in entries {
        let _ = writeln!(csv, "{key},data,string,{}", nvs_csv_escape(value));
    }
    fs::write(path, csv).with_context(|| format!("failed to write {}", path.display()))
}

fn nvs_csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn detect_esp_serial_port() -> Result<PathBuf> {
    let mut ports = Vec::new();
    let mut callout_ports = Vec::new();
    let dev = Path::new("/dev");
    for entry in fs::read_dir(dev).context("failed to read /dev for ESP serial ports")? {
        let entry = entry.context("failed to read /dev entry")?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_likely_esp_serial_port(&name) {
            let path = dev.join(&name);
            if name.starts_with("cu.") {
                callout_ports.push(path);
            } else {
                ports.push(path);
            }
        }
    }
    if !callout_ports.is_empty() {
        ports = callout_ports;
    }
    ports.sort();
    if ports.is_empty() {
        bail!("no ESP serial port detected; pass --port /dev/<device>");
    }
    if ports.len() > 1 {
        let mut message =
            String::from("multiple ESP serial ports detected; pass --port explicitly:");
        for port in ports {
            let _ = write!(message, "\n  {}", port.display());
        }
        bail!("{message}");
    }
    Ok(ports.remove(0))
}

fn is_likely_esp_serial_port(name: &str) -> bool {
    name.starts_with("cu.usbmodem")
        || name.starts_with("tty.usbmodem")
        || name.starts_with("cu.usbserial")
        || name.starts_with("tty.usbserial")
        || name.starts_with("cu.SLAB_USBtoUART")
        || name.starts_with("tty.SLAB_USBtoUART")
        || name.starts_with("cu.wchusbserial")
        || name.starts_with("tty.wchusbserial")
        || name.starts_with("ttyUSB")
        || name.starts_with("ttyACM")
}

fn run_process(command: &mut ProcessCommand, context: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("{context}: failed to start process"))?;
    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!(
        "{context}: exit status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout.trim(),
        stderr.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::nvs_csv_escape;

    #[test]
    fn nvs_csv_escape_leaves_plain_values_unchanged() {
        assert_eq!(nvs_csv_escape("sensor-001"), "sensor-001");
    }

    #[test]
    fn nvs_csv_escape_quotes_commas_and_quotes() {
        assert_eq!(nvs_csv_escape("ssid,\"lab\""), "\"ssid,\"\"lab\"\"\"");
    }
}
