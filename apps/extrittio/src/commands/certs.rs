use std::{fs, path::Path};

use anyhow::{Context, Result};
use reqwest::Method;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{CertsCommand, CertsSubcommand},
    models::{CaCertificate, CertificateBundle, CertificateStatus},
    output::{OutputFormat, output},
};

pub(super) async fn handle(
    command: CertsCommand,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    match command.command {
        CertsSubcommand::Ca => {
            let cert: CaCertificate = client.get("/api/v1/ca/certificate").await?;
            output(output_format, &cert, || cert.certificate_pem.clone())?;
        }
        CertsSubcommand::Download(args) => {
            let bundle = download_device_cert(client, &args.device_id, false).await?;
            handle_cert_bundle(output_format, bundle, args.out_dir.as_deref())?;
        }
        CertsSubcommand::Regenerate(args) => {
            let bundle = download_device_cert(client, &args.device_id, true).await?;
            handle_cert_bundle(output_format, bundle, args.out_dir.as_deref())?;
        }
        CertsSubcommand::Status { device_id } => {
            let status: Option<CertificateStatus> = client
                .get(&format!("/api/v1/devices/{device_id}/certificate/status"))
                .await?;
            output(output_format, &status, || match status {
                Some(ref s) => format!(
                    "fingerprint={}\nexpires_at={}\ncreated_at={}",
                    s.fingerprint, s.expires_at, s.created_at
                ),
                None => "No certificate".to_string(),
            })?;
        }
    }

    Ok(())
}

pub(super) async fn download_device_cert(
    client: &ApiClient,
    device_id: &str,
    regenerate: bool,
) -> Result<CertificateBundle> {
    let path = if regenerate {
        format!("/api/v1/devices/{device_id}/certificate/regenerate")
    } else {
        format!("/api/v1/devices/{device_id}/certificate")
    };
    let method = if regenerate {
        Method::POST
    } else {
        Method::GET
    };
    client.request(method, &path, None, true).await
}

pub(super) fn write_cert_bundle(out_dir: &Path, bundle: &CertificateBundle) -> Result<()> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    fs::write(out_dir.join("ca.pem"), &bundle.ca_pem)
        .with_context(|| format!("failed to write {}", out_dir.join("ca.pem").display()))?;
    fs::write(out_dir.join("device.pem"), &bundle.certificate_pem)
        .with_context(|| format!("failed to write {}", out_dir.join("device.pem").display()))?;
    let key_path = out_dir.join("device-key.pem");
    fs::write(&key_path, &bundle.private_key_pem)
        .with_context(|| format!("failed to write {}", key_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", key_path.display()))?;
    }

    Ok(())
}

fn handle_cert_bundle(
    output_format: OutputFormat,
    bundle: CertificateBundle,
    out_dir: Option<&Path>,
) -> Result<()> {
    if let Some(out_dir) = out_dir {
        write_cert_bundle(out_dir, &bundle)?;
        let value = json!({
            "fingerprint": bundle.fingerprint,
            "expires_at": bundle.expires_at,
            "created_at": bundle.created_at,
            "files": {
                "ca": out_dir.join("ca.pem"),
                "certificate": out_dir.join("device.pem"),
                "private_key": out_dir.join("device-key.pem"),
            }
        });
        output(output_format, &value, || {
            format!(
                "Wrote certificate bundle to {}\nfingerprint={}\nexpires_at={}",
                out_dir.display(),
                value["fingerprint"].as_str().unwrap_or_default(),
                value["expires_at"].as_str().unwrap_or_default()
            )
        })?;
    } else {
        output(output_format, &bundle, || {
            format!(
                "{}\n{}\n{}",
                bundle.certificate_pem, bundle.private_key_pem, bundle.ca_pem
            )
        })?;
    }
    Ok(())
}
