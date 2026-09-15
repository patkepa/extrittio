use anyhow::{Context, Result, ensure};
use extrittio_client_contract::ProvisionedContract;
use serde_json::json;
use std::{io::Write, path::Path};

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
    ensure!(
        !args.contract_out.as_os_str().is_empty(),
        "Contract output path cannot be empty"
    );
    match std::fs::symlink_metadata(&args.contract_out) {
        Ok(_) => anyhow::bail!(
            "Contract output already exists: {}",
            args.contract_out.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Cannot inspect contract output"),
    }
    let parent = args
        .contract_out
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // Reserve a private file in the destination filesystem before creating a device.
    let mut contract_file =
        tempfile::NamedTempFile::new_in(parent).context("Cannot prepare contract output")?;
    let device = create_device(client, args.device.clone()).await?;
    let contract_response: serde_json::Value = client.get(&format!("/api/v1/devices/{}/contract", device.id)).await
        .with_context(|| format!("Device {} was created, but contract download failed; retain this ID before retrying", device.id))?;
    let bytes = serde_json::to_vec_pretty(&contract_response)?;
    let contract = ProvisionedContract::from_api_response(&bytes)
        .with_context(|| format!("Invalid contract for created device {}", device.id))?;
    contract.validate_device_id(&device.id)?;
    ensure!(
        contract.document().blueprint_revision_id == args.device.blueprint_revision_id,
        "Contract revision differs from the requested revision for device {}",
        device.id
    );
    ensure!(
        contract_response["id"].as_str() == Some(contract.contract_id())
            && contract_response["device_id"].as_str() == Some(device.id.as_str())
            && contract_response["blueprint_revision_id"].as_str()
                == Some(args.device.blueprint_revision_id.as_str()),
        "Contract response identity differs from its verified document for device {}",
        device.id
    );
    let zenoh_endpoint = contract.zenoh_endpoint()?;
    contract_file.write_all(&bytes)?;
    contract_file.as_file().sync_all()?;
    contract_file
        .persist_noclobber(&args.contract_out)
        .with_context(|| {
            format!(
                "Cannot save contract for created device {} to {}",
                device.id,
                args.contract_out.display()
            )
        })?;
    let cert_written = if let Some(cert_dir) = args.cert_dir.as_deref() {
        let bundle = download_device_cert(client, &device.id, args.regenerate_cert).await?;
        write_cert_bundle(cert_dir, &bundle)?;
        Some(cert_dir.to_path_buf())
    } else {
        None
    };
    let esp32_nvs = if args.flash_esp32_nvs {
        let result = flash_esp32_nvs(&device, &args, zenoh_endpoint)?;
        Some(result)
    } else {
        None
    };
    let value = json!({
        "device_id": device.id,
        "name": device.name,
        "blueprint_revision_id": args.device.blueprint_revision_id,
        "fleet_id": device.fleet_id,
        "fleet_name": device.fleet_name,
        "firmware": device.firmware,
        "zenoh_connect": zenoh_endpoint,
        "contract_file": args.contract_out,
        "contract_id": contract.contract_id(),
        "contract_hash": contract.hash().to_string(),
        "certificate_dir": cert_written,
        "esp32_nvs": esp32_nvs,
    });
    output(output_format, &value, || format_provisioning(&value))?;

    Ok(())
}
