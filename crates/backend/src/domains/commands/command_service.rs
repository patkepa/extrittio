use std::sync::Arc;
use std::sync::atomic::Ordering;

use prost::Message;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::commands::port::CommandRepository;
use crate::domains::commands::types::{CommandQuery, CommandRecord, NewCommandRecord};
use crate::error::AppError;
use crate::state::ZenohMetrics;
use extrittio_backend_core::devices::DeviceRepository;
use extrittio_common::extrittio::DeviceCommand;

pub fn authorize_send_commands(ctx: &RequestContext) -> Result<(), AppError> {
    policy::require(ctx, Permission::SendCommands)
}

#[allow(clippy::implicit_hasher, clippy::too_many_arguments)]
pub async fn send_command_as_user_with_repository(
    ctx: &RequestContext,
    repository: &dyn CommandRepository,
    devices: &dyn DeviceRepository,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    command: &str,
    params: serde_json::Value,
    zenoh_metrics: &ZenohMetrics,
) -> Result<CommandRecord, AppError> {
    policy::require(ctx, Permission::SendCommands)?;
    let mut target_address = extrittio_common::topics::commands(device_id);
    let protocol_params = params
        .as_object()
        .ok_or_else(|| AppError::BadRequest("Command params must be a JSON object".into()))?
        .iter()
        .map(|(key, value)| {
            let value = value
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| value.to_string());
            (key.clone(), value)
        })
        .collect();
    if let Some(assigned) = devices
        .assigned_contract(ctx.tenant_id(), device_id)
        .await?
    {
        let contract: extrittio_device_contract::CompiledContractDocument =
            serde_json::from_value(assigned.document).map_err(|error| {
                AppError::Internal(format!("stored device contract is invalid: {error}"))
            })?;
        let definition = contract.commands.get(command).ok_or_else(|| {
            AppError::UnprocessableEntity(format!(
                "Command '{command}' is not declared by the device contract"
            ))
        })?;
        extrittio_device_contract::validate_instance(
            extrittio_device_contract::SchemaProfile::ExtrittioV1,
            &definition.input_schema,
            &params,
        )
        .map_err(|error| {
            AppError::UnprocessableEntity(format!(
                "Command '{command}' input failed contract validation: {error}"
            ))
        })?;
        let route = contract
            .routes
            .get(&definition.request_route)
            .ok_or_else(|| AppError::Internal("command request route is missing".into()))?;
        let transport = contract
            .transports
            .get(&route.transport)
            .ok_or_else(|| AppError::Internal("command transport is missing".into()))?;
        if transport.protocol != extrittio_device_contract::TransportProtocol::Zenoh {
            return Err(AppError::UnprocessableEntity(format!(
                "Command '{command}' requires an unavailable {:?} transport adapter",
                transport.protocol
            )));
        }
        target_address = route.address.clone();
    }
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let params_json = serde_json::to_string(&params)?;
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
        params: protocol_params,
        correlation_id,
    }
    .encode_to_vec();
    zenoh_session
        .put(target_address, payload)
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
