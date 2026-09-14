#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

pub mod application;
pub mod fleets;
pub use application::FleetApplication;
pub mod device_types;
pub use application::DeviceTypeApplication;
pub mod bootstrap;
pub mod certificates;
pub use application::BootstrapApplication;
pub use application::{CertificateApplication, CertificateSystemApplication};
pub mod ci_ingest;
pub mod context;
pub use application::CiIngestApplication;
pub use ci_ingest::{
    CiIngestOutcome, CiIngestParams, CiIngestRepository, authorize_ci_device_type,
};
pub mod error;
pub mod pagination;
pub mod ports;
mod repositories;
pub mod roles;
pub mod shadows;
pub mod users;
pub mod zones;

pub use application::{
    Application, ApplicationDependencies, AuthenticatedUser, CreateRole, CreateUser, CreateZone,
    MIN_PASSWORD_LEN, RoleApplication, RoleUpdate, UserApplication, ZoneApplication, ZoneUpdate,
    authenticated_user_from_details, primary_role_name, validate_password,
};
pub use context::{Actor, Permission, PermissionSet, TenantContext, TenantId, TenantIdError};
pub use error::{ApplicationError, ConstraintName, PersistenceError};
pub use pagination::{Page, PageRequest, PageRequestError};
pub use ports::{Clock, PasswordHasher, PasswordHasherError};
pub use repositories::{RepositorySet, RepositorySetInput};
pub use roles::{
    ADMIN_ROLE, DeleteRoleOutcome, NewRole, OPERATOR_ROLE, OWNER_ROLE, Role, RoleDetails,
    RolePatch, RoleRepository, UpdateRoleOutcome, VIEWER_ROLE,
};
pub use users::{
    ChangePasswordOutcome, CreateUserOutcome, DeleteUserOutcome, EncodedPasswordHash, NewUser,
    RecordSuccessfulLoginOutcome, SetUserRolesOutcome, User, UserAuthEpoch, UserCredentials,
    UserDetails, UserPage, UserRepository,
};
pub use zones::{
    DeleteZoneOutcome, NewZone, RuleZoneSnapshotRepository, Zone, ZonePatch, ZoneRepository,
};

pub mod api_keys;
pub use api_keys::{
    ApiKeyGenerator, ApiKeyRecord, ApiKeyRepository, ApiKeySummary, CreateApiKey,
    CreateApiKeyRecord, CreatedApiKey, GeneratedApiKey,
};
pub use application::ApiKeyApplication;

pub mod device_blueprints;
pub mod device_contracts;
pub use application::DeviceBlueprintApplication;

pub mod devices;
pub use application::{DeviceApplication, DeviceTargetSelection, ProvisionDevice};

pub mod rules;
pub use extrittio_rule_engine as rule_engine;

pub mod rule_snapshots;

pub mod outbox;
pub mod rule_actions;
pub use application::{OutboxApplication, OutboxWorkerApplication};

pub mod alerts;
pub use application::{
    AlertApplication, AlertMaintenanceApplication, AlertWorkerApplication, RuleAlertIntent,
};

pub use application::RuleRuntimeApplication;

pub use application::{DeviceShadowApplication, ShadowApplication};

pub mod configuration;
pub use application::ConfigurationApplication;

pub mod commands;
pub use application::CommandWorkerApplication;

pub use application::CommandApplication;

pub mod logs;
pub use application::{LogApplication, LogIngressApplication};
