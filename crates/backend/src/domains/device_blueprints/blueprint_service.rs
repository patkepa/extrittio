use chrono::Utc;
use extrittio_device_contract::{
    BlueprintCompiler, Compatibility, CompileContext, ContractError, DeviceBlueprint,
    ResolvedTransport, TransportProtocol, ValidatedBlueprint, compare_blueprints,
    validate_blueprint,
};
use serde_json::Value;
use uuid::Uuid;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::devices::types::NewDeviceContractRecord;
use crate::error::AppError;
use crate::persistence::PersistenceError;

use super::repository::DeviceBlueprintRepository;
use super::types::{
    BlueprintDraftRecord, BlueprintList, BlueprintRecord, BlueprintRevisionRecord,
    CreateBlueprintRecord, PublishBlueprintOutcome, PublishBlueprintRecord,
    ReplaceBlueprintDraftRecord,
};

#[derive(Debug, Clone)]
pub struct BlueprintValidation {
    pub valid: bool,
    pub issues: Vec<extrittio_device_contract::ValidationIssue>,
}

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    limit: i64,
    offset: i64,
) -> Result<BlueprintList, AppError> {
    policy::require(ctx, Permission::ReadDeviceBlueprints)?;
    Ok(repository.list(ctx.tenant_id(), limit, offset).await?)
}

pub async fn get(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    blueprint_id: &str,
) -> Result<BlueprintRecord, AppError> {
    policy::require(ctx, Permission::ReadDeviceBlueprints)?;
    repository
        .get(ctx.tenant_id(), blueprint_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device blueprint '{blueprint_id}' not found")))
}

pub async fn create(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    document: Value,
) -> Result<(BlueprintRecord, BlueprintDraftRecord), AppError> {
    policy::require(ctx, Permission::ManageDeviceBlueprints)?;
    let blueprint = parse_document(&document)?;
    validate_draft_identity(&blueprint)?;
    let result = repository
        .create(
            ctx.tenant_id(),
            CreateBlueprintRecord {
                id: Uuid::new_v4().to_string(),
                draft_id: Uuid::new_v4().to_string(),
                key: blueprint.metadata.key,
                name: blueprint.metadata.name,
                description: blueprint.metadata.description,
                document,
                now: Utc::now(),
            },
        )
        .await
        .map_err(map_unique_key)?;
    Ok(result)
}

pub async fn get_draft(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    blueprint_id: &str,
) -> Result<BlueprintDraftRecord, AppError> {
    policy::require(ctx, Permission::ReadDeviceBlueprints)?;
    repository
        .get_draft(ctx.tenant_id(), blueprint_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device blueprint '{blueprint_id}' not found")))
}

pub async fn replace_draft(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    blueprint_id: &str,
    document: Value,
) -> Result<BlueprintDraftRecord, AppError> {
    policy::require(ctx, Permission::ManageDeviceBlueprints)?;
    let blueprint = parse_document(&document)?;
    validate_draft_identity(&blueprint)?;
    repository
        .replace_draft(
            ctx.tenant_id(),
            blueprint_id,
            ReplaceBlueprintDraftRecord {
                key: blueprint.metadata.key,
                name: blueprint.metadata.name,
                description: blueprint.metadata.description,
                document,
                now: Utc::now(),
            },
        )
        .await
        .map_err(map_unique_key)?
        .ok_or_else(|| AppError::NotFound(format!("Device blueprint '{blueprint_id}' not found")))
}

pub async fn validate_draft(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    blueprint_id: &str,
) -> Result<BlueprintValidation, AppError> {
    policy::require(ctx, Permission::ReadDeviceBlueprints)?;
    let draft = repository
        .get_draft(ctx.tenant_id(), blueprint_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("Device blueprint '{blueprint_id}' not found"))
        })?;
    Ok(validation_result(draft.document))
}

pub async fn latest_revision(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    blueprint_id: &str,
) -> Result<BlueprintRevisionRecord, AppError> {
    policy::require(ctx, Permission::ReadDeviceBlueprints)?;
    repository
        .latest_revision(ctx.tenant_id(), blueprint_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Device blueprint '{blueprint_id}' has no published revision"
            ))
        })
}

pub async fn get_revision(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    revision_id: &str,
) -> Result<BlueprintRevisionRecord, AppError> {
    policy::require(ctx, Permission::ReadDeviceBlueprints)?;
    repository
        .get_revision(ctx.tenant_id(), revision_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Device blueprint revision '{revision_id}' not found"
            ))
        })
}

pub async fn publish(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    blueprint_id: &str,
) -> Result<BlueprintRevisionRecord, AppError> {
    policy::require(ctx, Permission::ManageDeviceBlueprints)?;
    let draft = repository
        .get_draft(ctx.tenant_id(), blueprint_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("Device blueprint '{blueprint_id}' not found"))
        })?;
    let validated = validate_document(draft.document.clone())?;
    let compatibility = match repository
        .latest_revision(ctx.tenant_id(), blueprint_id)
        .await?
    {
        Some(previous) => {
            let previous = validate_document(previous.document).map_err(|error| {
                AppError::Internal(format!("stored blueprint revision is invalid: {error}"))
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
    let compatibility = serde_json::to_value(compatibility)?;
    match repository
        .publish(
            ctx.tenant_id(),
            blueprint_id,
            PublishBlueprintRecord {
                revision_id: Uuid::new_v4().to_string(),
                expected_draft_updated_at: draft.updated_at,
                document: draft.document,
                document_hash,
                compatibility,
                now: Utc::now(),
            },
        )
        .await?
    {
        PublishBlueprintOutcome::Published(revision) => Ok(revision),
        PublishBlueprintOutcome::BlueprintNotFound => Err(AppError::NotFound(format!(
            "Device blueprint '{blueprint_id}' not found"
        ))),
        PublishBlueprintOutcome::DraftChanged => Err(AppError::Conflict(
            "Blueprint draft changed during publication; validate and publish it again".to_string(),
        )),
    }
}

pub async fn compile_device_contract(
    ctx: &RequestContext,
    repository: &dyn DeviceBlueprintRepository,
    revision_id: &str,
    contract_id: String,
    device_id: String,
    automatic_zenoh_endpoint: String,
    configuration: Option<Value>,
) -> Result<NewDeviceContractRecord, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;
    let revision = repository
        .get_revision(ctx.tenant_id(), revision_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Device blueprint revision '{revision_id}' not found"
            ))
        })?;
    let blueprint = validate_document(revision.document).map_err(|error| {
        AppError::Internal(format!(
            "stored device blueprint revision '{}' is invalid: {error}",
            revision.id
        ))
    })?;
    let blueprint_revision = u32::try_from(revision.revision).map_err(|_| {
        AppError::Internal(format!(
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
            tenant_id: ctx.tenant_id_str().to_string(),
            device_id,
            blueprint_revision_id: revision.id.clone(),
            blueprint_revision,
            transport_bindings,
            configuration_layers: configuration.into_iter().collect(),
        },
    )
    .map_err(|error| {
        AppError::UnprocessableEntity(format!("Device contract compilation failed: {error}"))
    })?;
    Ok(NewDeviceContractRecord {
        id: contract_id,
        blueprint_revision_id: revision.id,
        document: serde_json::to_value(&compiled.document)?,
        contract_hash: compiled.hash.to_string(),
        created_at: Utc::now(),
    })
}

fn parse_document(document: &Value) -> Result<DeviceBlueprint, AppError> {
    serde_json::from_value(document.clone())
        .map_err(|error| AppError::BadRequest(format!("Invalid blueprint document: {error}")))
}

fn validate_document(document: Value) -> Result<ValidatedBlueprint, AppError> {
    let blueprint = parse_document(&document)?;
    validate_blueprint(blueprint).map_err(|error| {
        let details = error
            .validation_issues()
            .unwrap_or_default()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        AppError::UnprocessableEntity(format!("Blueprint validation failed: {details}"))
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

fn validate_draft_identity(blueprint: &DeviceBlueprint) -> Result<(), AppError> {
    if blueprint.metadata.key.trim().is_empty() || blueprint.metadata.key.len() > 64 {
        return Err(AppError::BadRequest(
            "Blueprint metadata.key must contain 1-64 bytes".to_string(),
        ));
    }
    if blueprint.metadata.name.trim().is_empty() || blueprint.metadata.name.len() > 128 {
        return Err(AppError::BadRequest(
            "Blueprint metadata.name must contain 1-128 bytes".to_string(),
        ));
    }
    Ok(())
}

fn map_unique_key(error: PersistenceError) -> AppError {
    match error {
        PersistenceError::UniqueViolation { .. } => {
            AppError::Conflict("A device blueprint with this key already exists".to_string())
        }
        other => other.into(),
    }
}

fn contract_internal(error: ContractError) -> AppError {
    AppError::Internal(format!(
        "failed to canonicalize validated blueprint: {error}"
    ))
}
