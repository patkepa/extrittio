use crate::auth::context::RequestContext;
use crate::error::AppError;

pub use extrittio_backend_core::Permission;

pub fn require(ctx: &RequestContext, permission: Permission) -> Result<(), AppError> {
    if ctx.has_permission(permission) {
        Ok(())
    } else if ctx.user_id > 0 {
        Err(AppError::Forbidden(format!(
            "Missing permission '{}'",
            permission.key()
        )))
    } else {
        Err(AppError::Unauthorized)
    }
}

#[must_use]
pub fn satisfies(held_permission: Permission, required_permission: Permission) -> bool {
    held_permission.satisfies(required_permission)
}

#[allow(dead_code)]
pub fn require_legacy(ctx: &RequestContext, permission: Permission) -> Result<(), AppError> {
    match permission {
        Permission::ReadFirmware => require_authenticated(ctx),
        Permission::ReadCommands => require_authenticated(ctx),
        Permission::ReadAlerts => require_authenticated(ctx),
        Permission::ReadDeviceBlueprints => require_authenticated(ctx),
        Permission::ReadDevices => require_authenticated(ctx),
        Permission::ReadFleets => require_authenticated(ctx),
        Permission::ReadLogs => require_authenticated(ctx),
        Permission::ReadRules => require_authenticated(ctx),
        Permission::ReadServerMetrics => require_admin(ctx),
        Permission::ReadShadows => require_authenticated(ctx),
        Permission::ReadTelemetry => require_authenticated(ctx),
        Permission::ReadZones => require_authenticated(ctx),
        Permission::DeployFirmware => require_admin(ctx),
        Permission::ManageAlerts => require_admin(ctx),
        Permission::ManageDeviceBlueprints => require_admin(ctx),
        Permission::ManageApiKeys | Permission::ManageFirmware => require_admin(ctx),
        Permission::ManageDevices => require_admin(ctx),
        Permission::ManageFleets => require_admin(ctx),
        Permission::ManageRules => require_admin(ctx),
        Permission::ManageRoles | Permission::ReadRoles => require_admin(ctx),
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
