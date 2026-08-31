use std::sync::Arc;

use crate::{RoleRepository, RuleZoneSnapshotRepository, ZoneRepository};

/// Named business-port dependencies used to construct a [`RepositorySet`].
///
/// Adapters construct this value after they have created their concrete
/// repositories. Database connection, migration, health, backup, and other
/// lifecycle capabilities intentionally do not belong here.
pub struct RepositorySetInput {
    pub roles: Arc<dyn RoleRepository>,
    pub zones: Arc<dyn ZoneRepository>,
    pub rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
}

/// Complete set of business persistence ports available to the application.
///
/// Fields stay private so transports and runtime code cannot use this as a
/// service locator. Only application modules decompose the set into the narrow
/// use-case façades they own.
#[derive(Clone)]
pub struct RepositorySet {
    roles: Arc<dyn RoleRepository>,
    zones: Arc<dyn ZoneRepository>,
    rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
}

impl RepositorySet {
    #[must_use]
    pub fn new(input: RepositorySetInput) -> Self {
        Self {
            roles: input.roles,
            zones: input.zones,
            rule_zone_snapshots: input.rule_zone_snapshots,
        }
    }

    pub(crate) fn into_parts(self) -> RepositorySetParts {
        RepositorySetParts {
            roles: self.roles,
            zones: self.zones,
            rule_zone_snapshots: self.rule_zone_snapshots,
        }
    }
}

pub(crate) struct RepositorySetParts {
    pub(crate) roles: Arc<dyn RoleRepository>,
    pub(crate) zones: Arc<dyn ZoneRepository>,
    pub(crate) rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::{
        DeleteRoleOutcome, DeleteZoneOutcome, NewRole, NewZone, PersistenceError, RoleDetails,
        RolePatch, TenantId, UpdateRoleOutcome, Zone, ZonePatch,
    };

    struct FakeZoneRepository;

    #[async_trait]
    impl ZoneRepository for FakeZoneRepository {
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

    struct FakeRuleZoneSnapshotRepository;

    struct FakeRoleRepository;

    #[async_trait]
    impl RoleRepository for FakeRoleRepository {
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

    #[async_trait]
    impl RuleZoneSnapshotRepository for FakeRuleZoneSnapshotRepository {
        async fn list_for_rule_snapshot(&self) -> Result<Vec<Zone>, PersistenceError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn construction_requires_only_the_migrated_business_ports() {
        let zones = Arc::new(FakeZoneRepository);
        let rule_zone_snapshots = Arc::new(FakeRuleZoneSnapshotRepository);
        let roles = Arc::new(FakeRoleRepository);

        let repositories = RepositorySet::new(RepositorySetInput {
            roles: roles.clone(),
            zones: zones.clone(),
            rule_zone_snapshots: rule_zone_snapshots.clone(),
        });
        let parts = repositories.into_parts();

        assert!(Arc::ptr_eq(
            &parts.roles,
            &(roles as Arc<dyn RoleRepository>)
        ));

        assert!(Arc::ptr_eq(
            &parts.zones,
            &(zones as Arc<dyn ZoneRepository>)
        ));
        assert!(Arc::ptr_eq(
            &parts.rule_zone_snapshots,
            &(rule_zone_snapshots as Arc<dyn RuleZoneSnapshotRepository>)
        ));
    }
}
