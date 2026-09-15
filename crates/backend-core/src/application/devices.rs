use super::{
    CertificateApplication, DeviceBlueprintApplication, require_permission,
};
use crate::devices::*;
use crate::{ApplicationError, Clock, Permission, PersistenceError, TenantContext};
use std::sync::Arc;

pub struct ProvisionDevice {
    pub name: String,
    pub blueprint_revision_id: String,
    pub fleet_id: Option<i32>,
    pub firmware: Option<String>,
    pub configuration: Option<serde_json::Value>,
    pub automatic_zenoh_endpoint: String,
}

/// Keep the first occurrence, so response ordering follows selection ordering.
fn normalize_device_ids(ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

pub struct DeviceTargetSelection<'a> {
    pub device_ids: Option<&'a [String]>,
    pub select_all: bool,
    pub status: Option<&'a str>,
    pub search: Option<&'a str>,
    pub fleet_id: Option<i32>,
    pub max_size: usize,
}

fn map_device_write_error(error: PersistenceError) -> ApplicationError {
    match error {
        PersistenceError::UniqueViolation { .. } => {
            ApplicationError::Conflict("A device with this name already exists".into())
        }
        other => other.into(),
    }
}

#[derive(Clone)]
pub struct DeviceApplication {
    repository: Arc<dyn DeviceRepository>,
    blueprints: DeviceBlueprintApplication,
    certificates: CertificateApplication,
    clock: Arc<dyn Clock>,
}
impl DeviceApplication {
    pub fn new(
        repository: Arc<dyn DeviceRepository>,
        blueprints: DeviceBlueprintApplication,
        certificates: CertificateApplication,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            blueprints,
            certificates,
            clock,
        }
    }

    /// Prepare all external values before entering the atomic database operation.
    pub async fn provision(
        &self,
        ctx: &TenantContext,
        request: ProvisionDevice,
    ) -> Result<DeviceDetails, ApplicationError> {
        if request.name.trim().is_empty() {
            return Err(ApplicationError::InvalidInput(
                "Device name must not be empty".into(),
            ));
        }
        require_permission(ctx, Permission::ManageDevices)?;
        let id = uuid::Uuid::new_v4().to_string();
        let contract = self
            .blueprints
            .compile_device_contract(
                ctx,
                &request.blueprint_revision_id,
                uuid::Uuid::new_v4().to_string(),
                id.clone(),
                request.automatic_zenoh_endpoint,
                request.configuration,
            )
            .await?;
        self.create(
            ctx,
            CreateDeviceRecord {
                id,
                name: request.name,
                fleet_id: request.fleet_id,
                firmware: request.firmware.unwrap_or_else(|| "unknown".into()),
                contract,
            },
        )
        .await
    }

    pub async fn list(
        &self,
        ctx: &TenantContext,
        query: DeviceListQuery,
    ) -> Result<(Vec<DeviceDetails>, i64), ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        let result = self.repository.list(ctx.tenant_id(), query).await?;
        Ok((result.records, result.total))
    }

    pub async fn get(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<DeviceDetails, ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        self.repository
            .get(ctx.tenant_id(), device_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))
    }

    pub async fn assigned_contract(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<DeviceContractRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        self.repository
            .assigned_contract(ctx.tenant_id(), device_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(format!("Device '{device_id}' has no assigned contract"))
            })
    }

    async fn create(
        &self,
        ctx: &TenantContext,
        record: CreateDeviceRecord,
    ) -> Result<DeviceDetails, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        let certificate = self.certificates.prepare_device(ctx, &record.id).await?;
        self.repository
            .create(ctx.tenant_id(), record, certificate)
            .await
            .map_err(map_device_write_error)
    }

    pub async fn update(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        mut record: UpdateDeviceRecord,
    ) -> Result<DeviceDetails, ApplicationError> {
        if record
            .name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err(ApplicationError::InvalidInput(
                "Device name must not be empty".into(),
            ));
        }
        require_permission(ctx, Permission::ManageDevices)?;
        record.updated_at = Some(self.clock.now());
        self.repository
            .update(ctx.tenant_id(), device_id, record)
            .await
            .map_err(map_device_write_error)?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))
    }

    pub async fn resolve_target_ids(
        &self,
        ctx: &TenantContext,
        selection: DeviceTargetSelection<'_>,
    ) -> Result<Vec<String>, ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        let ids = if selection.select_all {
            self.repository
                .resolve_ids(
                    ctx.tenant_id(),
                    DeviceFilter {
                        status: selection.status.map(ToOwned::to_owned),
                        search: selection.search.map(ToOwned::to_owned),
                        fleet_id: selection.fleet_id,
                    },
                )
                .await?
        } else {
            selection
                .device_ids
                .ok_or_else(|| {
                    ApplicationError::InvalidInput(
                        "Either device_ids or select_all with filters is required".into(),
                    )
                })?
                .to_vec()
        };
        let ids = normalize_device_ids(ids);
        if ids.len() > selection.max_size {
            return Err(ApplicationError::InvalidInput(format!(
                "Too many devices ({}). Maximum is {}. Narrow your filters.",
                ids.len(),
                selection.max_size
            )));
        }
        Ok(ids)
    }

    pub async fn delete(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<(), ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        if !self.repository.delete(ctx.tenant_id(), device_id).await? {
            return Err(ApplicationError::NotFound(format!(
                "Device '{device_id}' not found"
            )));
        }
        Ok(())
    }

    pub async fn bulk_assign_fleet(
        &self,
        ctx: &TenantContext,
        device_ids: Vec<String>,
        fleet_id: Option<i32>,
    ) -> Result<usize, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        Ok(self
            .repository
            .bulk_assign_fleet(
                ctx.tenant_id(),
                normalize_device_ids(device_ids),
                fleet_id,
                self.clock.now(),
            )
            .await?)
    }

    pub async fn bulk_delete(
        &self,
        ctx: &TenantContext,
        device_ids: Vec<String>,
    ) -> Result<usize, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        Ok(self
            .repository
            .bulk_delete(ctx.tenant_id(), normalize_device_ids(device_ids))
            .await?)
    }
}
