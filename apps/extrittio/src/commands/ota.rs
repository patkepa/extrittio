use std::{fmt::Write as _, time::Duration};

use anyhow::{Result, anyhow, bail};
use reqwest::Method;
use serde_json::json;
use tokio::time::{Instant, sleep};

use crate::{
    api::ApiClient,
    args::{DeployOtaArgs, OtaCommand, OtaSubcommand, OtaWaitArgs},
    models::{BulkResultResponse, OtaDeploymentResponse, Paginated},
    output::{OutputFormat, output},
};

pub(super) async fn handle(
    command: OtaCommand,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    match command.command {
        OtaSubcommand::Deploy(args) => deploy(args, output_format, client).await?,
        OtaSubcommand::Status(args) => {
            let deployments =
                get_deployments(client, &args.device, args.limit, args.offset).await?;
            output(output_format, &deployments, || {
                format_deployments(&deployments)
            })?;
        }
        OtaSubcommand::Wait(args) => wait_for_deployment(args, output_format, client).await?,
    }

    Ok(())
}

async fn deploy(
    args: DeployOtaArgs,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    if let Some(device_id) = args.device {
        if args.all || args.fleet_id.is_some() || args.status.is_some() || args.search.is_some() {
            bail!("--device cannot be combined with --all or filters");
        }
        client
            .request_empty_json(
                Method::POST,
                &format!("/api/v1/devices/{device_id}/ota"),
                json!({ "firmware_update_id": args.firmware_id }),
            )
            .await?;
        let value = json!({
            "device_id": device_id,
            "firmware_update_id": args.firmware_id,
            "triggered": true,
        });
        output(output_format, &value, || {
            format!(
                "Triggered OTA for device {} with firmware {}",
                value["device_id"].as_str().unwrap_or_default(),
                value["firmware_update_id"].as_i64().unwrap_or_default()
            )
        })?;
        return Ok(());
    }

    if !args.all && args.fleet_id.is_none() && args.status.is_none() && args.search.is_none() {
        bail!("pass --device, --all, or at least one filter such as --fleet-id");
    }

    let result: BulkResultResponse = client
        .request(
            Method::POST,
            "/api/v1/devices/bulk/ota",
            Some(json!({
                "select_all": true,
                "filters": {
                    "status": args.status,
                    "search": args.search,
                    "fleet_id": args.fleet_id,
                },
                "firmware_update_id": args.firmware_id,
            })),
            true,
        )
        .await?;
    output(output_format, &result, || format_bulk_result(&result))?;
    Ok(())
}

async fn wait_for_deployment(
    args: OtaWaitArgs,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(args.timeout_secs);
    let interval = Duration::from_secs(args.interval_secs.max(1));

    loop {
        let deployments = get_deployments(client, &args.device, 20, 0).await?;
        if let Some(deployment) = deployments
            .data
            .iter()
            .find(|deployment| deployment.firmware_update_id == args.firmware_id)
            && is_terminal(&deployment.status)
        {
            output(output_format, deployment, || format_deployment(deployment))?;
            return if deployment.status.eq_ignore_ascii_case("success") {
                Ok(())
            } else {
                Err(anyhow!(
                    "OTA failed for device {}: {}",
                    deployment.device_id,
                    deployment
                        .error_message
                        .as_deref()
                        .unwrap_or("no error reported")
                ))
            };
        }

        if Instant::now() >= deadline {
            bail!(
                "timed out waiting for firmware {} on device {}",
                args.firmware_id,
                args.device
            );
        }
        sleep(interval).await;
    }
}

async fn get_deployments(
    client: &ApiClient,
    device_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Paginated<OtaDeploymentResponse>> {
    client
        .get(&format!(
            "/api/v1/devices/{device_id}/ota-deployments?limit={limit}&offset={offset}"
        ))
        .await
}

fn format_deployments(deployments: &Paginated<OtaDeploymentResponse>) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<8} {:<28} {:<10} {:<12} STARTED",
        "ID", "FIRMWARE", "FW_ID", "STATUS"
    );
    for deployment in &deployments.data {
        let _ = writeln!(
            out,
            "{:<8} {:<28} {:<10} {:<12} {}",
            deployment.id,
            truncate(&deployment.firmware_version, 28),
            deployment.firmware_update_id,
            deployment.status,
            deployment.initiated_at
        );
    }
    let _ = write!(
        out,
        "\nshowing {} of {} (limit={}, offset={})",
        deployments.data.len(),
        deployments.total,
        deployments.limit,
        deployments.offset
    );
    out
}

fn format_deployment(deployment: &OtaDeploymentResponse) -> String {
    format!(
        "id={}\ndevice={}\nfirmware={} ({})\nstatus={}\nerror={}\ninitiated_at={}\ncompleted_at={}",
        deployment.id,
        deployment.device_id,
        deployment.firmware_version,
        deployment.firmware_update_id,
        deployment.status,
        deployment.error_message.as_deref().unwrap_or("-"),
        deployment.initiated_at,
        deployment.completed_at.as_deref().unwrap_or("-"),
    )
}

fn format_bulk_result(result: &BulkResultResponse) -> String {
    let mut out = format!("succeeded={}\nfailed={}", result.succeeded, result.failed);
    for error in &result.errors {
        let _ = write!(out, "\n{}: {}", error.device_id, error.error);
    }
    out
}

fn is_terminal(status: &str) -> bool {
    status.eq_ignore_ascii_case("success") || status.eq_ignore_ascii_case("failed")
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
