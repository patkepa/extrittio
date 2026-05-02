use anyhow::Result;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::ProvisionArgs,
    esp32::flash_esp32_nvs,
    output::{OutputFormat, format_provisioning, output},
};

use super::{
    certs::{download_device_cert, write_cert_bundle},
    devices::create_device,
};

pub(super) async fn handle(
    args: ProvisionArgs,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    let device = create_device(client, args.device.clone()).await?;
    let cert_written = if let Some(cert_dir) = args.cert_dir.as_deref() {
        let bundle = download_device_cert(client, &device.id, args.regenerate_cert).await?;
        write_cert_bundle(cert_dir, &bundle)?;
        Some(cert_dir.to_path_buf())
    } else {
        None
    };
    let esp32_nvs = if args.flash_esp32_nvs {
        let result = flash_esp32_nvs(&device, &args)?;
        Some(result)
    } else {
        None
    };
    let value = json!({
        "device_id": device.id,
        "name": device.name,
        "device_type_id": device.device_type_id,
        "device_type_name": device.device_type_name,
        "fleet_id": device.fleet_id,
        "fleet_name": device.fleet_name,
        "firmware": device.firmware,
        "zenoh_connect": args.zenoh_connect,
        "certificate_dir": cert_written,
        "esp32_nvs": esp32_nvs,
    });
    output(output_format, &value, || format_provisioning(&value))?;

    Ok(())
}
