use super::require_permission;
use crate::device_types::*;
use crate::{ApplicationError, Permission, TenantContext};
use std::sync::Arc;

const DEFAULT_ICON: &str = "cube";
const DEFAULT_COLOR_HEX: &str = "#8ABBFF";
const LEGACY_DEFAULT_TYPE: &str = "default";

fn validate_name(name: &str) -> Result<String, ApplicationError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApplicationError::InvalidInput(
            "Device type name must not be empty".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_icon(icon: &str) -> Result<String, ApplicationError> {
    let trimmed = icon.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(ApplicationError::InvalidInput(
            "Device type icon must be 1-64 characters".into(),
        ));
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(ApplicationError::InvalidInput(
            "Device type icon must use lowercase letters, digits, hyphen, or underscore".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_color_hex(color_hex: &str) -> Result<String, ApplicationError> {
    let trimmed = color_hex.trim();
    let valid = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if !valid {
        return Err(ApplicationError::InvalidInput(
            "Device type color must be a #RRGGBB hex value".into(),
        ));
    }
    Ok(trimmed.to_ascii_uppercase())
}

#[derive(Clone)]
pub struct DeviceTypeApplication {
    repository: Arc<dyn DeviceTypeRepository>,
}
impl DeviceTypeApplication {
    pub fn new(repository: Arc<dyn DeviceTypeRepository>) -> Self {
        Self { repository }
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<DeviceTypeRecord>, i64), ApplicationError> {
        require_permission(ctx, Permission::ReadDeviceTypes)?;
        let result = self.repository.list(ctx.tenant_id(), limit, offset).await?;
        Ok((result.records, result.total))
    }

    /// Resolve the compatibility-only storage type used while firmware and older
    /// APIs are migrated to blueprint selectors. New callers should not expose
    /// this implementation detail to device creators.
    pub async fn resolve_for_device_creation(
        &self,
        ctx: &TenantContext,
        requested_id: Option<i32>,
    ) -> Result<DeviceTypeRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageDevices)?;
        let record = match requested_id {
            Some(id) => self.repository.get_by_id(ctx.tenant_id(), id).await?,
            None => {
                self.repository
                    .get_by_name(ctx.tenant_id(), LEGACY_DEFAULT_TYPE)
                    .await?
            }
        };
        record.ok_or_else(|| {
            ApplicationError::InvalidOperation(match requested_id {
                Some(id) => format!("Device type {id} not found"),
                None => "Tenant has no default compatibility device type".into(),
            })
        })
    }

    pub async fn create(
        &self,
        ctx: &TenantContext,
        name: &str,
        icon: Option<&str>,
        color_hex: Option<&str>,
    ) -> Result<DeviceTypeRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageDeviceTypes)?;

        let record = CreateDeviceTypeRecord {
            name: validate_name(name)?,
            icon: validate_icon(icon.unwrap_or(DEFAULT_ICON))?,
            color_hex: validate_color_hex(color_hex.unwrap_or(DEFAULT_COLOR_HEX))?,
        };
        Ok(self.repository.create(ctx.tenant_id(), record).await?)
    }

    pub async fn update(
        &self,
        ctx: &TenantContext,
        id: i32,
        name: Option<&str>,
        icon: Option<&str>,
        color_hex: Option<&str>,
    ) -> Result<DeviceTypeRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageDeviceTypes)?;

        let changes = UpdateDeviceTypeRecord {
            name: name.map(validate_name).transpose()?,
            icon: icon.map(validate_icon).transpose()?,
            color_hex: color_hex.map(validate_color_hex).transpose()?,
        };

        let record =
            if changes.name.is_none() && changes.icon.is_none() && changes.color_hex.is_none() {
                self.repository.get_by_id(ctx.tenant_id(), id).await?
            } else {
                self.repository.update(ctx.tenant_id(), id, changes).await?
            };
        record.ok_or_else(|| ApplicationError::NotFound(format!("Device type {id} not found")))
    }

    pub async fn delete(&self, ctx: &TenantContext, id: i32) -> Result<(), ApplicationError> {
        require_permission(ctx, Permission::ManageDeviceTypes)?;

        if id == 1 {
            return Err(ApplicationError::InvalidOperation(
                "Cannot delete the default device type".into(),
            ));
        }

        match self
            .repository
            .delete_if_unused(ctx.tenant_id(), id)
            .await?
        {
            DeleteDeviceTypeOutcome::Deleted => Ok(()),
            DeleteDeviceTypeOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "Device type {id} not found"
            ))),
            DeleteDeviceTypeOutcome::InUse { device_count } => Err(ApplicationError::Conflict(
                format!("Cannot delete device type: {device_count} device(s) still reference it"),
            )),
        }
    }
}
