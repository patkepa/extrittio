use super::require_permission;
use crate::{ApplicationError, Clock, Permission, PersistenceError, TenantContext};
use extrittio_device_contract::{
    BlueprintCompiler, Compatibility, CompileContext, ContractError, DeviceBlueprint,
    ResolvedTransport, TransportProtocol, ValidatedBlueprint, compare_blueprints,
    validate_blueprint,
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::device_contracts::NewDeviceContractRecord;

use crate::device_blueprints::DeviceBlueprintRepository;
use crate::device_blueprints::{
    BlueprintDraftRecord, BlueprintList, BlueprintRecord, BlueprintRevisionRecord,
    CreateBlueprintRecord, PublishBlueprintOutcome, PublishBlueprintRecord,
    ReplaceBlueprintDraftRecord,
};

#[derive(Debug, Clone)]
pub struct BlueprintValidation {
    pub valid: bool,
    pub issues: Vec<extrittio_device_contract::ValidationIssue>,
}

#[derive(Clone)]
pub struct DeviceBlueprintApplication {
    repository: Arc<dyn DeviceBlueprintRepository>,
    clock: Arc<dyn Clock>,
}
impl DeviceBlueprintApplication {
    pub fn new(repository: Arc<dyn DeviceBlueprintRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn list(
        &self,
        ctx: &TenantContext,
        limit: i64,
        offset: i64,
    ) -> Result<BlueprintList, ApplicationError> {
        require_permission(ctx, Permission::ReadDeviceBlueprints)?;
        Ok(self.repository.list(ctx.tenant_id(), limit, offset).await?)
    }

    pub async fn get(
        &self,
        ctx: &TenantContext,
        blueprint_id: &str,
    ) -> Result<BlueprintRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadDeviceBlueprints)?;
        self.repository
            .get(ctx.tenant_id(), blueprint_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!("Device blueprint '{blueprint_id}' not found"))
            })
    }

    pub async fn create(
        &self,
        ctx: &TenantContext,
        document: Value,
    ) -> Result<(BlueprintRecord, BlueprintDraftRecord), ApplicationError> {
        require_permission(ctx, Permission::ManageDeviceBlueprints)?;
        let blueprint = parse_document(&document)?;
        validate_draft_identity(&blueprint)?;
        let result = self
            .repository
            .create(
                ctx.tenant_id(),
                CreateBlueprintRecord {
                    id: Uuid::new_v4().to_string(),
                    draft_id: Uuid::new_v4().to_string(),
                    key: blueprint.metadata.key,
                    name: blueprint.metadata.name,
                    description: blueprint.metadata.description,
                    document,
                    now: self.clock.now(),
                },
            )
            .await
            .map_err(map_unique_key)?;
        Ok(result)
    }

    pub async fn get_draft(
        &self,
        ctx: &TenantContext,
        blueprint_id: &str,
    ) -> Result<BlueprintDraftRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadDeviceBlueprints)?;
        self.repository
            .get_draft(ctx.tenant_id(), blueprint_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!("Device blueprint '{blueprint_id}' not found"))
            })
    }

    pub async fn replace_draft(
        &self,
        ctx: &TenantContext,
        blueprint_id: &str,
        document: Value,
    ) -> Result<BlueprintDraftRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageDeviceBlueprints)?;
        let blueprint = parse_document(&document)?;
        validate_draft_identity(&blueprint)?;
        self.repository
            .replace_draft(
                ctx.tenant_id(),
                blueprint_id,
                ReplaceBlueprintDraftRecord {
                    key: blueprint.metadata.key,
                    name: blueprint.metadata.name,
                    description: blueprint.metadata.description,
                    document,
                    now: self.clock.now(),
                },
            )
            .await
            .map_err(map_unique_key)?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!("Device blueprint '{blueprint_id}' not found"))
            })
    }

    pub async fn validate_draft(
        &self,
        ctx: &TenantContext,
        blueprint_id: &str,
    ) -> Result<BlueprintValidation, ApplicationError> {
        require_permission(ctx, Permission::ReadDeviceBlueprints)?;
        let draft = self
            .repository
            .get_draft(ctx.tenant_id(), blueprint_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!("Device blueprint '{blueprint_id}' not found"))
            })?;
        Ok(validation_result(draft.document))
    }

    pub async fn latest_revision(
        &self,
        ctx: &TenantContext,
        blueprint_id: &str,
    ) -> Result<BlueprintRevisionRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadDeviceBlueprints)?;
        self.repository
            .latest_revision(ctx.tenant_id(), blueprint_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "Device blueprint '{blueprint_id}' has no published revision"
                ))
            })
    }

    pub async fn get_revision(
        &self,
        ctx: &TenantContext,
        revision_id: &str,
    ) -> Result<BlueprintRevisionRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadDeviceBlueprints)?;
        self.repository
            .get_revision(ctx.tenant_id(), revision_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "Device blueprint revision '{revision_id}' not found"
                ))
            })
    }

    pub async fn publish(
        &self,
        ctx: &TenantContext,
        blueprint_id: &str,
    ) -> Result<BlueprintRevisionRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageDeviceBlueprints)?;
        let draft = self
            .repository
            .get_draft(ctx.tenant_id(), blueprint_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!("Device blueprint '{blueprint_id}' not found"))
            })?;
        let validated = validate_document(draft.document.clone())?;
        let previous = self
            .repository
            .latest_revision(ctx.tenant_id(), blueprint_id)
            .await?;
        let expected_previous_revision_id = previous.as_ref().map(|revision| revision.id.clone());
        let compatibility = match previous {
            Some(previous) => {
                let previous = validate_document(previous.document).map_err(|error| {
                    ApplicationError::Internal(format!(
                        "stored blueprint revision is invalid: {error}"
                    ))
                })?;
                compare_blueprints(&previous, &validated)
            }
            None => Compatibility {
                breaking: false,
                changes: Vec::new(),
            },
        };
        let document_hash = validated
            .document_hash()
            .map_err(contract_internal)?
            .to_string();
        let compatibility = serde_json::to_value(compatibility).map_err(serialization_error)?;
        match self
            .repository
            .publish(
                ctx.tenant_id(),
                blueprint_id,
                PublishBlueprintRecord {
                    revision_id: Uuid::new_v4().to_string(),
                    expected_previous_revision_id,
                    expected_draft_updated_at: draft.updated_at,
                    document: draft.document,
                    document_hash,
                    compatibility,
                    now: self.clock.now(),
                },
            )
            .await?
        {
            PublishBlueprintOutcome::Published(revision) => Ok(revision),
            PublishBlueprintOutcome::BlueprintNotFound => Err(ApplicationError::NotFound(format!(
                "Device blueprint '{blueprint_id}' not found"
            ))),
            PublishBlueprintOutcome::PublicationChanged => Err(ApplicationError::Conflict(
                "Blueprint publication changed concurrently; publish it again".to_string(),
            )),
            PublishBlueprintOutcome::DraftChanged => Err(ApplicationError::Conflict(
                "Blueprint draft changed during publication; validate and publish it again"
                    .to_string(),
            )),
        }
    }

    pub(crate) async fn compile_device_contract(
        &self,
        ctx: &TenantContext,
        revision_id: &str,
        contract_id: String,
        device_id: String,
        automatic_zenoh_endpoint: String,
        configuration: Option<Value>,
    ) -> Result<NewDeviceContractRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        let revision = self
            .repository
            .get_revision(ctx.tenant_id(), revision_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!(
                    "Device blueprint revision '{revision_id}' not found"
                ))
            })?;
        let blueprint = validate_document(revision.document).map_err(|error| {
            ApplicationError::Internal(format!(
                "stored device blueprint revision '{}' is invalid: {error}",
                revision.id
            ))
        })?;
        let blueprint_revision = u32::try_from(revision.revision).map_err(|_| {
            ApplicationError::Internal(format!(
                "stored device blueprint revision '{}' has an invalid number",
                revision.id
            ))
        })?;
        let transport_bindings = blueprint
            .blueprint()
            .spec
            .transports
            .iter()
            .filter(|transport| transport.protocol == TransportProtocol::Zenoh)
            .map(|transport| {
                (
                    transport.binding.clone(),
                    ResolvedTransport {
                        protocol: TransportProtocol::Zenoh,
                        endpoint: automatic_zenoh_endpoint.clone(),
                        server_ca_pem: None,
                        credential_ref: Some("device-certificate".to_string()),
                    },
                )
            })
            .collect();
        let compiled = BlueprintCompiler::compile(
            &blueprint,
            &CompileContext {
                contract_id: contract_id.clone(),
                tenant_id: ctx.tenant_id().as_str().to_string(),
                device_id,
                blueprint_revision_id: revision.id.clone(),
                blueprint_revision,
                transport_bindings,
                configuration_layers: configuration.into_iter().collect(),
            },
        )
        .map_err(|error| {
            ApplicationError::InvalidOperation(format!(
                "Device contract compilation failed: {error}"
            ))
        })?;
        Ok(NewDeviceContractRecord {
            id: contract_id,
            blueprint_revision_id: revision.id,
            document: serde_json::to_value(&compiled.document).map_err(serialization_error)?,
            initial_configuration: compiled
                .document
                .configuration
                .as_ref()
                .map(|configuration| configuration.desired.clone()),
            contract_hash: compiled.hash.to_string(),
            created_at: self.clock.now(),
        })
    }
}

fn serialization_error(error: serde_json::Error) -> ApplicationError {
    ApplicationError::Internal(error.to_string())
}

fn parse_document(document: &Value) -> Result<DeviceBlueprint, ApplicationError> {
    serde_json::from_value(document.clone()).map_err(|error| {
        ApplicationError::InvalidInput(format!("Invalid blueprint document: {error}"))
    })
}

fn validate_document(document: Value) -> Result<ValidatedBlueprint, ApplicationError> {
    let blueprint = parse_document(&document)?;
    validate_blueprint(blueprint).map_err(|error| {
        let details = error
            .validation_issues()
            .unwrap_or_default()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        ApplicationError::InvalidOperation(format!("Blueprint validation failed: {details}"))
    })
}

fn validation_result(document: Value) -> BlueprintValidation {
    match serde_json::from_value::<DeviceBlueprint>(document) {
        Ok(blueprint) => match validate_blueprint(blueprint) {
            Ok(_) => BlueprintValidation {
                valid: true,
                issues: Vec::new(),
            },
            Err(error) => BlueprintValidation {
                valid: false,
                issues: error.validation_issues().unwrap_or_default().to_vec(),
            },
        },
        Err(error) => BlueprintValidation {
            valid: false,
            issues: vec![extrittio_device_contract::ValidationIssue::new(
                "/",
                error.to_string(),
            )],
        },
    }
}

fn validate_draft_identity(blueprint: &DeviceBlueprint) -> Result<(), ApplicationError> {
    if blueprint.metadata.key.trim().is_empty() || blueprint.metadata.key.len() > 64 {
        return Err(ApplicationError::InvalidInput(
            "Blueprint metadata.key must contain 1-64 bytes".to_string(),
        ));
    }
    if blueprint.metadata.name.trim().is_empty() || blueprint.metadata.name.len() > 128 {
        return Err(ApplicationError::InvalidInput(
            "Blueprint metadata.name must contain 1-128 bytes".to_string(),
        ));
    }
    Ok(())
}

fn map_unique_key(error: PersistenceError) -> ApplicationError {
    match error {
        PersistenceError::UniqueViolation { .. } => ApplicationError::Conflict(
            "A device blueprint with this key already exists".to_string(),
        ),
        other => other.into(),
    }
}

fn contract_internal(error: ContractError) -> ApplicationError {
    ApplicationError::Internal(format!(
        "failed to canonicalize validated blueprint: {error}"
    ))
}
