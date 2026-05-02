use crate::auth::context::RequestContext;
use crate::error::AppError;

#[derive(Debug, Clone, Copy)]
pub enum Permission {
    DeployFirmware,
    ManageAlerts,
    ManageDeviceTypes,
    ManageDevices,
    ManageApiKeys,
    ManageFirmware,
    ManageFleets,
    ManageRules,
    ManageShadows,
    ManageUsers,
    ManageZones,
    ReadCommands,
    ReadAlerts,
    ReadDeviceTypes,
    ReadDevices,
    ReadFleets,
    ReadFirmware,
    ReadLogs,
    ReadRules,
    ReadShadows,
    ReadTelemetry,
    ReadUsers,
    ReadZones,
    SendCommands,
}

pub fn require(ctx: &RequestContext, permission: Permission) -> Result<(), AppError> {
    match permission {
        Permission::ReadFirmware => require_authenticated(ctx),
        Permission::ReadCommands => require_authenticated(ctx),
        Permission::ReadAlerts => require_authenticated(ctx),
        Permission::ReadDeviceTypes => require_authenticated(ctx),
        Permission::ReadDevices => require_authenticated(ctx),
        Permission::ReadFleets => require_authenticated(ctx),
        Permission::ReadLogs => require_authenticated(ctx),
        Permission::ReadRules => require_authenticated(ctx),
        Permission::ReadShadows => require_authenticated(ctx),
        Permission::ReadTelemetry => require_authenticated(ctx),
        Permission::ReadZones => require_authenticated(ctx),
        Permission::DeployFirmware => require_admin(ctx),
        Permission::ManageAlerts => require_admin(ctx),
        Permission::ManageApiKeys | Permission::ManageFirmware => require_admin(ctx),
        Permission::ManageDeviceTypes => require_admin(ctx),
        Permission::ManageDevices => require_admin(ctx),
        Permission::ManageFleets => require_admin(ctx),
        Permission::ManageRules => require_admin(ctx),
        Permission::ManageShadows => require_admin(ctx),
        Permission::ManageUsers | Permission::ReadUsers => require_admin(ctx),
        Permission::ManageZones => require_admin(ctx),
        Permission::SendCommands => require_admin(ctx),
    }
}

fn require_authenticated(ctx: &RequestContext) -> Result<(), AppError> {
    if ctx.user_id > 0 {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

fn require_admin(ctx: &RequestContext) -> Result<(), AppError> {
    if ctx.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden("Admin role required".into()))
    }
}
