use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Validated identifier for data owned by one tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(value: impl Into<String>) -> Result<Self, TenantIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(TenantIdError::Empty);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for TenantId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TenantIdError {
    #[error("tenant id must not be empty")]
    Empty,
}

/// Authenticated actor supplied by a host transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    User {
        id: i32,
        username: String,
        role: String,
    },
    ApiKey {
        id: String,
    },
    Device {
        id: String,
    },
    System {
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Permission {
    DeployFirmware,
    ManageAlerts,
    ManageApiKeys,
    ManageDeviceBlueprints,
    ManageDeviceTypes,
    ManageDevices,
    ManageFirmware,
    ManageFleets,
    ManageRoles,
    ManageRules,
    ManageShadows,
    ManageUsers,
    ManageZones,
    ReadAlerts,
    ReadCommands,
    ReadDeviceBlueprints,
    ReadDeviceTypes,
    ReadDevices,
    ReadFirmware,
    ReadFleets,
    ReadLogs,
    ReadRoles,
    ReadRules,
    ReadServerMetrics,
    ReadShadows,
    ReadTelemetry,
    ReadUsers,
    ReadZones,
    SendCommands,
}

impl Permission {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::DeployFirmware => "firmware.deploy",
            Self::ManageAlerts => "alerts.manage",
            Self::ManageApiKeys => "api_keys.manage",
            Self::ManageDeviceBlueprints => "device_blueprints.manage",
            Self::ManageDeviceTypes => "device_types.manage",
            Self::ManageDevices => "devices.manage",
            Self::ManageFirmware => "firmware.manage",
            Self::ManageFleets => "fleets.manage",
            Self::ManageRoles => "roles.manage",
            Self::ManageRules => "rules.manage",
            Self::ManageShadows => "shadows.manage",
            Self::ManageUsers => "users.manage",
            Self::ManageZones => "zones.manage",
            Self::ReadAlerts => "alerts.read",
            Self::ReadCommands => "commands.read",
            Self::ReadDeviceBlueprints => "device_blueprints.read",
            Self::ReadDeviceTypes => "device_types.read",
            Self::ReadDevices => "devices.read",
            Self::ReadFirmware => "firmware.read",
            Self::ReadFleets => "fleets.read",
            Self::ReadLogs => "logs.read",
            Self::ReadRoles => "roles.read",
            Self::ReadRules => "rules.read",
            Self::ReadServerMetrics => "server_metrics.read",
            Self::ReadShadows => "shadows.read",
            Self::ReadTelemetry => "telemetry.read",
            Self::ReadUsers => "users.read",
            Self::ReadZones => "zones.read",
            Self::SendCommands => "commands.send",
        }
    }

    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        ALL_PERMISSIONS
            .iter()
            .copied()
            .find(|permission| permission.key() == key)
    }
}

const ALL_PERMISSIONS: &[Permission] = &[
    Permission::DeployFirmware,
    Permission::ManageAlerts,
    Permission::ManageApiKeys,
    Permission::ManageDeviceBlueprints,
    Permission::ManageDeviceTypes,
    Permission::ManageDevices,
    Permission::ManageFirmware,
    Permission::ManageFleets,
    Permission::ManageRoles,
    Permission::ManageRules,
    Permission::ManageShadows,
    Permission::ManageUsers,
    Permission::ManageZones,
    Permission::ReadAlerts,
    Permission::ReadCommands,
    Permission::ReadDeviceBlueprints,
    Permission::ReadDeviceTypes,
    Permission::ReadDevices,
    Permission::ReadFirmware,
    Permission::ReadFleets,
    Permission::ReadLogs,
    Permission::ReadRoles,
    Permission::ReadRules,
    Permission::ReadServerMetrics,
    Permission::ReadShadows,
    Permission::ReadTelemetry,
    Permission::ReadUsers,
    Permission::ReadZones,
    Permission::SendCommands,
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionSet(BTreeSet<Permission>);

impl PermissionSet {
    #[must_use]
    pub fn from_keys(keys: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Self(
            keys.into_iter()
                .filter_map(|key| Permission::from_key(key.as_ref()))
                .collect(),
        )
    }

    #[must_use]
    pub fn all() -> Self {
        Self(ALL_PERMISSIONS.iter().copied().collect())
    }

    #[must_use]
    pub fn contains(&self, required: Permission) -> bool {
        self.0.contains(&required)
            || implied_permissions(required)
                .iter()
                .any(|permission| self.0.contains(permission))
    }

    pub fn insert(&mut self, permission: Permission) -> bool {
        self.0.insert(permission)
    }
}

fn implied_permissions(permission: Permission) -> &'static [Permission] {
    match permission {
        Permission::ReadAlerts => &[Permission::ManageAlerts],
        Permission::ReadCommands => &[Permission::SendCommands],
        Permission::ReadDeviceBlueprints => &[Permission::ManageDeviceBlueprints],
        Permission::ReadDeviceTypes => &[Permission::ManageDeviceTypes],
        Permission::ReadDevices => &[Permission::ManageDevices],
        Permission::ReadFirmware => &[Permission::ManageFirmware, Permission::DeployFirmware],
        Permission::ReadFleets => &[Permission::ManageFleets],
        Permission::ReadRoles => &[Permission::ManageRoles],
        Permission::ReadRules => &[Permission::ManageRules],
        Permission::ReadShadows => &[Permission::ManageShadows],
        Permission::ReadUsers => &[Permission::ManageUsers],
        Permission::ReadZones => &[Permission::ManageZones],
        _ => &[],
    }
}

/// Non-optional, authenticated identity accepted by application use cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantContext {
    tenant_id: TenantId,
    actor: Actor,
    permissions: PermissionSet,
}

impl TenantContext {
    #[must_use]
    pub fn new(tenant_id: TenantId, actor: Actor, permissions: PermissionSet) -> Self {
        Self {
            tenant_id,
            actor,
            permissions,
        }
    }

    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    #[must_use]
    pub fn permissions(&self) -> &PermissionSet {
        &self.permissions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_deserialization_validates_the_identifier() {
        assert!(serde_json::from_str::<TenantId>(r#""tenant-a""#).is_ok());
        assert!(serde_json::from_str::<TenantId>(r#""  ""#).is_err());
    }

    #[test]
    fn management_permission_implies_matching_read_permission() {
        let permissions = PermissionSet::from_keys(["zones.manage"]);
        assert!(permissions.contains(Permission::ManageZones));
        assert!(permissions.contains(Permission::ReadZones));
    }
}
