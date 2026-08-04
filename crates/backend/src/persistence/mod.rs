use std::sync::Arc;

use crate::domains::configuration::repository::DeviceConfigRepository;
use crate::domains::dashboard::repository::DashboardReadRepository;
use crate::domains::device_types::repository::DeviceTypeRepository;
use crate::domains::devices::repository::DeviceRepository;
use crate::domains::fleets::repository::FleetRepository;
use crate::domains::identity::api_key_repository::ApiKeyRepository;
use crate::domains::identity::certificate_repository::CertificateRepository;
use crate::domains::identity::role_repository::RoleRepository;
use crate::domains::identity::user_repository::UserRepository;
use crate::domains::shadows::repository::ShadowRepository;

pub mod backend;
pub mod bootstrap;
pub mod error;
pub mod postgres;

pub use backend::{BackendCapabilities, BackendDescriptor, BackendKind};
pub use bootstrap::{
    BootstrapOwner, BootstrapRepository, BuiltinDeviceType, DatabaseHealth, SeedOwnerOutcome,
};
pub use error::{ConstraintName, PersistenceError};

/// Cloneable collection of backend-neutral persistence ports owned by
/// `AppState`. Additional domain ports are added as their PostgreSQL code is
/// extracted.
#[derive(Clone)]
pub struct Persistence {
    pub backend: BackendDescriptor,
    pub api_keys: Arc<dyn ApiKeyRepository>,
    pub bootstrap: Arc<dyn BootstrapRepository>,
    pub certificates: Arc<dyn CertificateRepository>,
    pub configuration: Arc<dyn DeviceConfigRepository>,
    pub dashboard: Arc<dyn DashboardReadRepository>,
    pub device_types: Arc<dyn DeviceTypeRepository>,
    pub devices: Arc<dyn DeviceRepository>,
    pub fleets: Arc<dyn FleetRepository>,
    pub roles: Arc<dyn RoleRepository>,
    pub shadows: Arc<dyn ShadowRepository>,
    pub users: Arc<dyn UserRepository>,
}

pub struct PersistencePorts {
    pub api_keys: Arc<dyn ApiKeyRepository>,
    pub bootstrap: Arc<dyn BootstrapRepository>,
    pub certificates: Arc<dyn CertificateRepository>,
    pub configuration: Arc<dyn DeviceConfigRepository>,
    pub dashboard: Arc<dyn DashboardReadRepository>,
    pub device_types: Arc<dyn DeviceTypeRepository>,
    pub devices: Arc<dyn DeviceRepository>,
    pub fleets: Arc<dyn FleetRepository>,
    pub roles: Arc<dyn RoleRepository>,
    pub shadows: Arc<dyn ShadowRepository>,
    pub users: Arc<dyn UserRepository>,
}

impl Persistence {
    #[must_use]
    pub fn new(backend: BackendDescriptor, ports: PersistencePorts) -> Self {
        Self {
            backend,
            api_keys: ports.api_keys,
            bootstrap: ports.bootstrap,
            certificates: ports.certificates,
            configuration: ports.configuration,
            dashboard: ports.dashboard,
            device_types: ports.device_types,
            devices: ports.devices,
            fleets: ports.fleets,
            roles: ports.roles,
            shadows: ports.shadows,
            users: ports.users,
        }
    }
}
