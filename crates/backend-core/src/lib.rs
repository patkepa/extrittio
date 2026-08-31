#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

pub mod application;
pub mod context;
pub mod error;
pub mod ports;
mod repositories;
pub mod roles;
pub mod zones;

pub use application::{
    Application, CreateRole, CreateZone, RoleApplication, RoleUpdate, ZoneApplication, ZoneUpdate,
};
pub use context::{Actor, Permission, PermissionSet, TenantContext, TenantId, TenantIdError};
pub use error::{ApplicationError, ConstraintName, PersistenceError};
pub use repositories::{RepositorySet, RepositorySetInput};
pub use roles::{
    ADMIN_ROLE, DeleteRoleOutcome, NewRole, OPERATOR_ROLE, OWNER_ROLE, Role, RoleDetails,
    RolePatch, RoleRepository, UpdateRoleOutcome, VIEWER_ROLE,
};
pub use zones::{
    DeleteZoneOutcome, NewZone, RuleZoneSnapshotRepository, Zone, ZonePatch, ZoneRepository,
};
