use std::collections::HashSet;

use crate::auth::context::RequestContext;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Permission {
    DeployFirmware,
    ManageAlerts,
    ManageDeviceTypes,
    ManageDevices,
    ManageApiKeys,
    ManageFirmware,
    ManageFleets,
    ManageRules,
    ManageRoles,
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
    ReadRoles,
    ReadServerMetrics,
    ReadShadows,
    ReadTelemetry,
    ReadUsers,
    ReadZones,
    SendCommands,
}

const ALL_PERMISSIONS: &[Permission] = &[
    Permission::DeployFirmware,
    Permission::ManageAlerts,
    Permission::ManageDeviceTypes,
    Permission::ManageDevices,
    Permission::ManageApiKeys,
    Permission::ManageFirmware,
    Permission::ManageFleets,
    Permission::ManageRules,
    Permission::ManageRoles,
    Permission::ManageShadows,
    Permission::ManageUsers,
    Permission::ManageZones,
    Permission::ReadCommands,
    Permission::ReadAlerts,
    Permission::ReadDeviceTypes,
    Permission::ReadDevices,
    Permission::ReadFleets,
    Permission::ReadFirmware,
    Permission::ReadLogs,
    Permission::ReadRules,
    Permission::ReadRoles,
    Permission::ReadServerMetrics,
    Permission::ReadShadows,
    Permission::ReadTelemetry,
    Permission::ReadUsers,
    Permission::ReadZones,
    Permission::SendCommands,
];

impl Permission {
    #[must_use]
    pub fn all() -> &'static [Permission] {
        ALL_PERMISSIONS
    }

    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Permission::DeployFirmware => "firmware.deploy",
            Permission::ManageAlerts => "alerts.manage",
            Permission::ManageDeviceTypes => "device_types.manage",
            Permission::ManageDevices => "devices.manage",
            Permission::ManageApiKeys => "api_keys.manage",
            Permission::ManageFirmware => "firmware.manage",
            Permission::ManageFleets => "fleets.manage",
            Permission::ManageRules => "rules.manage",
            Permission::ManageRoles => "roles.manage",
            Permission::ManageShadows => "shadows.manage",
            Permission::ManageUsers => "users.manage",
            Permission::ManageZones => "zones.manage",
            Permission::ReadCommands => "commands.read",
            Permission::ReadAlerts => "alerts.read",
            Permission::ReadDeviceTypes => "device_types.read",
            Permission::ReadDevices => "devices.read",
            Permission::ReadFleets => "fleets.read",
            Permission::ReadFirmware => "firmware.read",
            Permission::ReadLogs => "logs.read",
            Permission::ReadRules => "rules.read",
            Permission::ReadRoles => "roles.read",
            Permission::ReadServerMetrics => "server_metrics.read",
            Permission::ReadShadows => "shadows.read",
            Permission::ReadTelemetry => "telemetry.read",
            Permission::ReadUsers => "users.read",
            Permission::ReadZones => "zones.read",
            Permission::SendCommands => "commands.send",
        }
    }

    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "firmware.deploy" => Some(Permission::DeployFirmware),
            "alerts.manage" => Some(Permission::ManageAlerts),
            "device_types.manage" => Some(Permission::ManageDeviceTypes),
            "devices.manage" => Some(Permission::ManageDevices),
            "api_keys.manage" => Some(Permission::ManageApiKeys),
            "firmware.manage" => Some(Permission::ManageFirmware),
            "fleets.manage" => Some(Permission::ManageFleets),
            "rules.manage" => Some(Permission::ManageRules),
            "roles.manage" => Some(Permission::ManageRoles),
            "shadows.manage" => Some(Permission::ManageShadows),
            "users.manage" => Some(Permission::ManageUsers),
            "zones.manage" => Some(Permission::ManageZones),
            "commands.read" => Some(Permission::ReadCommands),
            "alerts.read" => Some(Permission::ReadAlerts),
            "device_types.read" => Some(Permission::ReadDeviceTypes),
            "devices.read" => Some(Permission::ReadDevices),
            "fleets.read" => Some(Permission::ReadFleets),
            "firmware.read" => Some(Permission::ReadFirmware),
            "logs.read" => Some(Permission::ReadLogs),
            "rules.read" => Some(Permission::ReadRules),
            "roles.read" => Some(Permission::ReadRoles),
            "server_metrics.read" => Some(Permission::ReadServerMetrics),
            "shadows.read" => Some(Permission::ReadShadows),
            "telemetry.read" => Some(Permission::ReadTelemetry),
            "users.read" => Some(Permission::ReadUsers),
            "zones.read" => Some(Permission::ReadZones),
            "commands.send" => Some(Permission::SendCommands),
            _ => None,
        }
    }

    #[must_use]
    pub fn from_keys(keys: &[String]) -> HashSet<Self> {
        keys.iter()
            .filter_map(|key| Permission::from_key(key))
            .collect()
    }
}

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

#[allow(dead_code)]
pub fn require_legacy(ctx: &RequestContext, permission: Permission) -> Result<(), AppError> {
    match permission {
        Permission::ReadFirmware => require_authenticated(ctx),
        Permission::ReadCommands => require_authenticated(ctx),
        Permission::ReadAlerts => require_authenticated(ctx),
        Permission::ReadDeviceTypes => require_authenticated(ctx),
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
        Permission::ManageApiKeys | Permission::ManageFirmware => require_admin(ctx),
        Permission::ManageDeviceTypes => require_admin(ctx),
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
