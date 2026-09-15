use crate::commands::{CommandRepository, CommandResponseStatus};
use crate::{ApplicationError, Clock, TenantId};
use std::sync::Arc;

/// Device responses and system-wide timeout maintenance share core transition policy.
#[derive(Clone)]
pub struct CommandWorkerApplication {
    repository: Arc<dyn CommandRepository>,
    clock: Arc<dyn Clock>,
}
impl CommandWorkerApplication {
    pub fn new(repository: Arc<dyn CommandRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn handle_response(
        &self,
        tenant: &TenantId,
        device_id: &str,
        correlation_id: &str,
        device_status: &str,
        payload: Option<String>,
    ) -> Result<Option<String>, ApplicationError> {
        if correlation_id.is_empty() {
            return Ok(None);
        }
        Ok(self
            .repository
            .apply_response(
                tenant,
                device_id,
                correlation_id.to_owned(),
                CommandResponseStatus::from_device_status(device_status),
                payload,
                self.clock.now().naive_utc(),
            )
            .await?)
    }
    pub async fn timeout_stale(&self, timeout_secs: u64) -> Result<usize, ApplicationError> {
        let now = self.clock.now().naive_utc();
        let cutoff = i64::try_from(timeout_secs)
            .ok()
            .and_then(chrono::TimeDelta::try_seconds)
            .and_then(|duration| now.checked_sub_signed(duration))
            .ok_or_else(|| {
                ApplicationError::InvalidInput(
                    "Command timeout is outside the supported range".into(),
                )
            })?;
        Ok(self.repository.timeout_stale(cutoff, now).await?)
    }
}

use super::require_permission;
use crate::commands::{CommandDelivery, CommandQuery, CommandRecord, DeviceBus, NewCommandRecord};
use crate::devices::DeviceRepository;
use crate::{Permission, TenantContext};

#[derive(Clone)]
pub struct CommandApplication {
    repository: Arc<dyn CommandRepository>,
    devices: Arc<dyn DeviceRepository>,
    bus: Arc<dyn DeviceBus>,
}
impl CommandApplication {
    pub fn new(
        repository: Arc<dyn CommandRepository>,
        devices: Arc<dyn DeviceRepository>,
        bus: Arc<dyn DeviceBus>,
    ) -> Self {
        Self {
            repository,
            devices,
            bus,
        }
    }
    pub fn authorize_send(&self, ctx: &TenantContext) -> Result<(), ApplicationError> {
        require_permission(ctx, Permission::SendCommands)
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        query: CommandQuery,
    ) -> Result<Vec<CommandRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadCommands)?;
        self.repository
            .list(ctx.tenant_id(), device_id, query)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))
    }
    pub async fn send(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        command: &str,
        params: serde_json::Value,
    ) -> Result<CommandRecord, ApplicationError> {
        // Preserve the HTTP endpoint's empty-command error precedence.
        if command.trim().is_empty() {
            return Err(ApplicationError::InvalidInput(
                "Command must not be empty".into(),
            ));
        }
        self.authorize_send(ctx)?;
        let address = self
            .validate(ctx.tenant_id(), device_id, command, &params)
            .await?;
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let record = self
            .repository
            .create(
                ctx.tenant_id(),
                device_id,
                NewCommandRecord {
                    id: correlation_id.clone(),
                    command: command.to_owned(),
                    params: params.to_string(),
                },
            )
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))?;
        self.bus
            .publish_command(CommandDelivery {
                device_id: device_id.to_owned(),
                command: command.to_owned(),
                params,
                correlation_id,
                address,
            })
            .await
            .map_err(|error| ApplicationError::DeviceCommunication(error.0))?;
        Ok(record)
    }

    /// Outbox delivery retries reuse their stable ID. Ordinary sends never call
    /// this operation and are never automatically republished.
    pub async fn deliver_action(
        &self,
        tenant: &TenantId,
        device_id: &str,
        delivery_id: &str,
        command: &str,
        params: serde_json::Value,
    ) -> Result<CommandRecord, ApplicationError> {
        if delivery_id.is_empty() || command.trim().is_empty() {
            return Err(ApplicationError::InvalidInput(
                "Command delivery ID and command must not be empty".into(),
            ));
        }
        // Preserve historical rule-action history's string-valued parameters.
        let params = if params.is_object() {
            params
        } else {
            serde_json::json!({})
        };
        let stored_params = serde_json::Value::Object(
            params
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        serde_json::Value::String(
                            value
                                .as_str()
                                .map(ToOwned::to_owned)
                                .unwrap_or_else(|| value.to_string()),
                        ),
                    )
                })
                .collect(),
        );
        let existing = self.repository.find(tenant, delivery_id).await?;
        if let Some(record) = existing.as_ref() {
            Self::validate_existing(record, device_id, command, &stored_params)?;
            if record.status != "sent" {
                return Ok(record.clone());
            }
        }
        let address = self.validate(tenant, device_id, command, &params).await?;
        let record = if let Some(record) = existing {
            record
        } else {
            match self
                .repository
                .create(
                    tenant,
                    device_id,
                    NewCommandRecord {
                        id: delivery_id.to_owned(),
                        command: command.to_owned(),
                        params: stored_params.to_string(),
                    },
                )
                .await
            {
                Ok(Some(record)) => record,
                Ok(None) => {
                    return Err(ApplicationError::NotFound(format!(
                        "Device '{device_id}' not found"
                    )));
                }
                Err(error) => {
                    // Another delivery may have inserted the same durable ID.
                    // Only an exact, tenant-scoped matching row makes this a retry.
                    let Some(record) = self.repository.find(tenant, delivery_id).await? else {
                        return Err(error.into());
                    };
                    Self::validate_existing(&record, device_id, command, &stored_params)?;
                    record
                }
            }
        };
        if record.status == "sent" {
            self.bus
                .publish_command(CommandDelivery {
                    device_id: device_id.to_owned(),
                    command: command.to_owned(),
                    params,
                    correlation_id: delivery_id.to_owned(),
                    address,
                })
                .await
                .map_err(|error| ApplicationError::DeviceCommunication(error.0))?;
        }
        Ok(record)
    }
    fn validate_existing(
        record: &CommandRecord,
        device_id: &str,
        command: &str,
        params: &serde_json::Value,
    ) -> Result<(), ApplicationError> {
        if record.device_id != device_id
            || record.command != command
            || serde_json::from_str::<serde_json::Value>(&record.params)
                .ok()
                .as_ref()
                != Some(params)
        {
            return Err(ApplicationError::Conflict(
                "Command delivery ID already belongs to a different command".into(),
            ));
        }
        Ok(())
    }
    async fn validate(
        &self,
        tenant: &TenantId,
        device_id: &str,
        command: &str,
        params: &serde_json::Value,
    ) -> Result<Option<String>, ApplicationError> {
        if !params.is_object() {
            return Err(ApplicationError::InvalidInput(
                "Command params must be a JSON object".into(),
            ));
        }
        let mut target_address = None;
        if let Some(assigned) = self.devices.assigned_contract(tenant, device_id).await? {
            let contract: extrittio_device_contract::CompiledContractDocument =
                serde_json::from_value(assigned.document).map_err(|error| {
                    ApplicationError::Internal(format!(
                        "stored device contract is invalid: {error}"
                    ))
                })?;
            let definition = contract.commands.get(command).ok_or_else(|| {
                ApplicationError::InvalidOperation(format!(
                    "Command '{command}' is not declared by the device contract"
                ))
            })?;
            extrittio_device_contract::validate_instance(
                extrittio_device_contract::SchemaProfile::ExtrittioV1,
                &definition.input_schema,
                params,
            )
            .map_err(|error| {
                ApplicationError::InvalidOperation(format!(
                    "Command '{command}' input failed contract validation: {error}"
                ))
            })?;
            let route = contract
                .routes
                .get(&definition.request_route)
                .ok_or_else(|| {
                    ApplicationError::Internal("command request route is missing".into())
                })?;
            let transport = contract
                .transports
                .get(&route.transport)
                .ok_or_else(|| ApplicationError::Internal("command transport is missing".into()))?;
            if !self.bus.supports(&transport.protocol) {
                return Err(ApplicationError::InvalidOperation(format!(
                    "Command '{command}' requires an unavailable {:?} transport adapter",
                    transport.protocol
                )));
            }
            target_address = Some(route.address.clone());
        }

        Ok(target_address)
    }
}
