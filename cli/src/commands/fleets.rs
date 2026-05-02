use anyhow::Result;
use reqwest::Method;
use serde_json::json;

use crate::{
    api::ApiClient,
    args::{FleetsCommand, FleetsSubcommand},
    models::{FleetResponse, Paginated},
    output::{OutputFormat, format_fleets, output},
};

use super::pagination::page_query;

pub(super) async fn handle(
    command: FleetsCommand,
    output_format: OutputFormat,
    client: &ApiClient,
) -> Result<()> {
    match command.command {
        FleetsSubcommand::List(args) => {
            let result: Paginated<FleetResponse> = client
                .get(&format!("/api/v1/fleets{}", page_query(&args)))
                .await?;
            output(output_format, &result, || format_fleets(&result))?;
        }
        FleetsSubcommand::Create { name } => {
            let result: FleetResponse = client
                .request(
                    Method::POST,
                    "/api/v1/fleets",
                    Some(json!({ "name": name })),
                    true,
                )
                .await?;
            output(output_format, &result, || {
                format!(
                    "{} {} devices={}",
                    result.id, result.name, result.device_count
                )
            })?;
        }
        FleetsSubcommand::Delete { id } => {
            client
                .request_empty(Method::DELETE, &format!("/api/v1/fleets/{id}"))
                .await?;
            let value = json!({ "deleted": true, "id": id });
            output(output_format, &value, || {
                format!("Deleted fleet {}", value["id"].as_i64().unwrap_or_default())
            })?;
        }
    }

    Ok(())
}
