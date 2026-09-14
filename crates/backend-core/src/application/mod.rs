mod alerts;
pub use alerts::{
    AlertApplication, AlertMaintenanceApplication, AlertWorkerApplication, RuleAlertIntent,
};
mod outbox;
pub use outbox::{OutboxApplication, OutboxWorkerApplication};
mod rules;
pub use rules::{RuleApplication, RuleRuntimeApplication};
mod devices;
pub use devices::{DeviceApplication, DeviceTargetSelection, ProvisionDevice};
mod device_blueprints;
pub use device_blueprints::{BlueprintValidation, DeviceBlueprintApplication};
mod api_keys;
mod fleets;
pub use fleets::FleetApplication;
mod device_types;
pub use device_types::DeviceTypeApplication;
mod bootstrap;
mod certificate_system;
mod certificates;
pub use bootstrap::BootstrapApplication;
mod ci_ingest;
pub use api_keys::ApiKeyApplication;
pub use certificate_system::CertificateSystemApplication;
pub use certificates::{CertBundle, CertificateApplication};
pub use ci_ingest::CiIngestApplication;
mod roles;
mod users;
mod zones;

use std::sync::Arc;

use crate::{Clock, PasswordHasher, Permission, RepositorySet, TenantContext};

pub use roles::{CreateRole, RoleApplication, RoleUpdate};
pub use users::{
    AuthenticatedUser, CreateUser, MIN_PASSWORD_LEN, UserApplication,
    authenticated_user_from_details, primary_role_name, validate_password,
};
pub use zones::{CreateZone, ZoneApplication, ZoneUpdate};

/// Named outbound dependencies used by application behavior.
///
/// Database lifecycle handles, transport clients, and runtime configuration
/// intentionally do not belong here.
#[derive(Clone)]
pub struct ApplicationDependencies {
    pub rule_changes: Arc<dyn crate::rules::RuleChangeNotifier>,
    pub webhook_urls: Arc<dyn crate::rules::WebhookUrlPolicy>,
    pub certificate_issuer: Arc<dyn crate::certificates::CertificateIssuer>,
    pub key_protector: Arc<dyn crate::certificates::KeyProtector>,
    pub api_key_generator: Arc<dyn crate::ApiKeyGenerator>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub clock: Arc<dyn Clock>,
}

impl ApplicationDependencies {
    #[must_use]
    pub fn new(
        password_hasher: Arc<dyn PasswordHasher>,
        clock: Arc<dyn Clock>,
        api_key_generator: Arc<dyn crate::ApiKeyGenerator>,
        certificate_issuer: Arc<dyn crate::certificates::CertificateIssuer>,
        key_protector: Arc<dyn crate::certificates::KeyProtector>,
        webhook_urls: Arc<dyn crate::rules::WebhookUrlPolicy>,
        rule_changes: Arc<dyn crate::rules::RuleChangeNotifier>,
    ) -> Self {
        Self {
            rule_changes,
            webhook_urls,
            password_hasher,
            clock,
            api_key_generator,
            certificate_issuer,
            key_protector,
        }
    }
}

/// Curated application façade passed to transports.
#[derive(Clone)]
pub struct Application {
    api_keys: ApiKeyApplication,
    device_blueprints: DeviceBlueprintApplication,
    devices: DeviceApplication,
    fleets: FleetApplication,
    device_types: DeviceTypeApplication,
    ci_ingest: CiIngestApplication,
    certificates: CertificateApplication,
    alerts: AlertApplication,
    outbox: OutboxApplication,
    rules: RuleApplication,
    roles: RoleApplication,
    users: UserApplication,
    zones: ZoneApplication,
}

impl Application {
    pub fn alerts(&self) -> &AlertApplication {
        &self.alerts
    }
    #[must_use]
    pub fn new(repositories: RepositorySet, dependencies: ApplicationDependencies) -> Self {
        let repositories = repositories.into_parts();
        let blueprints = DeviceBlueprintApplication::new(
            repositories.device_blueprints,
            dependencies.clock.clone(),
        );
        let device_types = DeviceTypeApplication::new(repositories.device_types);
        let certificates = CertificateApplication::new(
            repositories.certificates,
            dependencies.certificate_issuer,
            dependencies.key_protector,
        );
        let devices = DeviceApplication::new(
            repositories.devices,
            blueprints.clone(),
            device_types.clone(),
            certificates.clone(),
            dependencies.clock.clone(),
        );
        Self {
            alerts: AlertApplication::new(repositories.alerts),
            outbox: OutboxApplication::new(repositories.outbox),
            rules: RuleApplication::new(
                repositories.rules,
                dependencies.clock.clone(),
                dependencies.webhook_urls,
                dependencies.rule_changes,
            ),
            devices,
            api_keys: ApiKeyApplication::new(repositories.api_keys, dependencies.api_key_generator),
            ci_ingest: CiIngestApplication::new(repositories.ci_ingest),
            certificates,
            device_types,
            device_blueprints: blueprints,
            fleets: FleetApplication::new(repositories.fleets),
            roles: RoleApplication::new(repositories.roles),
            users: UserApplication::new(
                repositories.users,
                dependencies.password_hasher,
                dependencies.clock,
            ),
            zones: ZoneApplication::new(repositories.zones),
        }
    }

    #[must_use]
    pub fn certificates(&self) -> &CertificateApplication {
        &self.certificates
    }

    #[must_use]
    pub fn ci_ingest(&self) -> &CiIngestApplication {
        &self.ci_ingest
    }

    #[must_use]
    pub fn device_types(&self) -> &DeviceTypeApplication {
        &self.device_types
    }

    #[must_use]
    pub fn devices(&self) -> &DeviceApplication {
        &self.devices
    }

    #[must_use]
    pub fn device_blueprints(&self) -> &DeviceBlueprintApplication {
        &self.device_blueprints
    }

    #[must_use]
    pub fn fleets(&self) -> &FleetApplication {
        &self.fleets
    }

    #[must_use]
    pub fn api_keys(&self) -> &ApiKeyApplication {
        &self.api_keys
    }

    #[must_use]
    pub fn outbox(&self) -> &OutboxApplication {
        &self.outbox
    }

    #[must_use]
    pub fn rules(&self) -> &RuleApplication {
        &self.rules
    }

    #[must_use]
    pub fn roles(&self) -> &RoleApplication {
        &self.roles
    }

    #[must_use]
    pub fn users(&self) -> &UserApplication {
        &self.users
    }

    #[must_use]
    pub fn zones(&self) -> &ZoneApplication {
        &self.zones
    }
}

pub(crate) fn require_permission(
    context: &TenantContext,
    permission: Permission,
) -> Result<(), crate::ApplicationError> {
    if context.permissions().contains(permission) {
        Ok(())
    } else {
        Err(crate::ApplicationError::Forbidden(format!(
            "Missing permission '{}'",
            permission.key()
        )))
    }
}
