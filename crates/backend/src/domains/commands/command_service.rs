use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use prost::Message;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::commands::port::CommandRepository;
use crate::domains::commands::types::{CommandQuery, CommandRecord, NewCommandRecord};
use crate::error::AppError;
use crate::state::ZenohMetrics;
use crate::tenancy::DeviceIdentity;
use extrittio_common::extrittio::DeviceCommand;

pub fn authorize_send_commands(ctx: &RequestContext) -> Result<(), AppError> {
    policy::require(ctx, Permission::SendCommands)
}

#[allow(clippy::implicit_hasher)]
pub async fn send_command_as_user_with_repository(
    ctx: &RequestContext,
    repository: &dyn CommandRepository,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    command: &str,
    params: HashMap<String, String>,
    zenoh_metrics: &ZenohMetrics,
) -> Result<CommandRecord, AppError> {
    policy::require(ctx, Permission::SendCommands)?;
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let params_json = serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string());
    let record = repository
        .create(
            ctx.tenant_id(),
            device_id,
            NewCommandRecord {
                id: correlation_id.clone(),
                command: command.to_string(),
                params: params_json,
            },
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))?;

    let payload = DeviceCommand {
        command: command.to_string(),
        params,
        correlation_id,
    }
    .encode_to_vec();
    zenoh_session
        .put(extrittio_common::topics::commands(device_id), payload)
        .await
        .map_err(|error| AppError::Zenoh(error.to_string()))?;
    zenoh_metrics.messages_out.fetch_add(1, Ordering::Relaxed);
    Ok(record)
}

pub async fn list_commands_with_repository(
    ctx: &RequestContext,
    repository: &dyn CommandRepository,
    device_id: &str,
    query: CommandQuery,
) -> Result<Vec<CommandRecord>, AppError> {
    policy::require(ctx, Permission::ReadCommands)?;
    repository
        .list(ctx.tenant_id(), device_id, query)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}

pub async fn timeout_stale_with_repository(
    repository: &dyn CommandRepository,
    timeout_secs: u64,
) -> Result<usize, AppError> {
    #[allow(clippy::cast_possible_wrap)]
    let cutoff = chrono::Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);
    Ok(repository
        .timeout_stale(cutoff, chrono::Utc::now().naive_utc())
        .await?)
}

pub async fn handle_response_with_repository(
    repository: &dyn CommandRepository,
    identity: &DeviceIdentity,
    correlation_id: &str,
    device_status: &str,
    payload: Option<&str>,
) -> Result<Option<String>, AppError> {
    if correlation_id.is_empty() {
        return Ok(None);
    }
    Ok(repository
        .apply_response(
            identity,
            correlation_id.to_string(),
            device_status.to_string(),
            payload.map(ToOwned::to_owned),
        )
        .await?)
}
