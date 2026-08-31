mod roles;
mod zones;

use std::sync::Arc;

use crate::{Permission, RepositorySet, RuleZoneSnapshotRepository, TenantContext};

pub use roles::{CreateRole, RoleApplication, RoleUpdate};
pub use zones::{CreateZone, ZoneApplication, ZoneUpdate};

/// Curated application façade passed to transports.
#[derive(Clone)]
pub struct Application {
    roles: RoleApplication,
    zones: ZoneApplication,
    // Retained for the rules application slice. Keeping the port here ensures
    // `RepositorySet` is consumed by the application instead of becoming a
    // host-level service locator during the incremental migration.
    _rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
}

impl Application {
    #[must_use]
    pub fn new(repositories: RepositorySet) -> Self {
        let repositories = repositories.into_parts();
        Self {
            roles: RoleApplication::new(repositories.roles),
            zones: ZoneApplication::new(repositories.zones),
            _rule_zone_snapshots: repositories.rule_zone_snapshots,
        }
    }

    #[must_use]
    pub fn roles(&self) -> &RoleApplication {
        &self.roles
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

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::{
        DeleteRoleOutcome, DeleteZoneOutcome, NewRole, NewZone, PersistenceError,
        RepositorySetInput, RoleDetails, RolePatch, RoleRepository, TenantId, UpdateRoleOutcome,
        Zone, ZonePatch, ZoneRepository,
    };

    struct FakeRepositories;

    #[async_trait]
    impl ZoneRepository for FakeRepositories {
        async fn list(&self, _tenant: &TenantId) -> Result<Vec<Zone>, PersistenceError> {
            Ok(Vec::new())
        }

        async fn get(
            &self,
            _tenant: &TenantId,
            _zone_id: &str,
        ) -> Result<Option<Zone>, PersistenceError> {
            Ok(None)
        }

        async fn create(
            &self,
            _tenant: &TenantId,
            _zone: NewZone,
        ) -> Result<Zone, PersistenceError> {
            Err(PersistenceError::Internal("not used by this test".into()))
        }

        async fn update(
            &self,
            _tenant: &TenantId,
            _zone_id: &str,
            _patch: ZonePatch,
        ) -> Result<Option<Zone>, PersistenceError> {
            Ok(None)
        }

        async fn delete(
            &self,
            _tenant: &TenantId,
            _zone_id: &str,
        ) -> Result<DeleteZoneOutcome, PersistenceError> {
            Ok(DeleteZoneOutcome::NotFound)
        }
    }

    #[async_trait]
    impl RuleZoneSnapshotRepository for FakeRepositories {
        async fn list_for_rule_snapshot(&self) -> Result<Vec<Zone>, PersistenceError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl RoleRepository for FakeRepositories {
        async fn list(&self, _tenant: &TenantId) -> Result<Vec<RoleDetails>, PersistenceError> {
            Ok(Vec::new())
        }

        async fn create(
            &self,
            _tenant: &TenantId,
            _role: NewRole,
        ) -> Result<RoleDetails, PersistenceError> {
            Err(PersistenceError::Internal("not used by this test".into()))
        }

        async fn update(
            &self,
            _tenant: &TenantId,
            _role_id: i32,
            _patch: RolePatch,
        ) -> Result<UpdateRoleOutcome, PersistenceError> {
            Ok(UpdateRoleOutcome::NotFound)
        }

        async fn delete(
            &self,
            _tenant: &TenantId,
            _role_id: i32,
        ) -> Result<DeleteRoleOutcome, PersistenceError> {
            Ok(DeleteRoleOutcome::NotFound)
        }
    }

    #[test]
    fn application_consumes_and_retains_the_complete_repository_set() {
        let repository = Arc::new(FakeRepositories);
        let application = Application::new(RepositorySet::new(RepositorySetInput {
            roles: repository.clone(),
            zones: repository.clone(),
            rule_zone_snapshots: repository.clone(),
        }));

        // The caller, zone façade, and retained snapshot port each hold one
        // reference. No database runtime or lifecycle handle is required.
        assert_eq!(Arc::strong_count(&repository), 4);
        let _roles = application.roles();
        let _zones = application.zones();
    }
}
