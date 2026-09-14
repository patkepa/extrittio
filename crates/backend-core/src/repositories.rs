use std::sync::Arc;

use crate::{RoleRepository, RuleZoneSnapshotRepository, UserRepository, ZoneRepository};

/// Named business-port dependencies used to construct a [`RepositorySet`].
///
/// Adapters construct this value after they have created their concrete
/// repositories. Database connection, migration, health, backup, and other
/// lifecycle capabilities intentionally do not belong here.
pub struct RepositorySetInput {
    pub api_keys: Arc<dyn crate::ApiKeyRepository>,
    pub ci_ingest: Arc<dyn crate::CiIngestRepository>,
    pub roles: Arc<dyn RoleRepository>,
    pub users: Arc<dyn UserRepository>,
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
    api_keys: Arc<dyn crate::ApiKeyRepository>,
    ci_ingest: Arc<dyn crate::CiIngestRepository>,
    roles: Arc<dyn RoleRepository>,
    users: Arc<dyn UserRepository>,
    zones: Arc<dyn ZoneRepository>,
    rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
}

impl RepositorySet {
    #[must_use]
    pub fn new(input: RepositorySetInput) -> Self {
        Self {
            api_keys: input.api_keys,
            ci_ingest: input.ci_ingest,
            roles: input.roles,
            users: input.users,
            zones: input.zones,
            rule_zone_snapshots: input.rule_zone_snapshots,
        }
    }

    pub(crate) fn into_parts(self) -> RepositorySetParts {
        RepositorySetParts {
            api_keys: self.api_keys,
            ci_ingest: self.ci_ingest,
            roles: self.roles,
            users: self.users,
            zones: self.zones,
            rule_zone_snapshots: self.rule_zone_snapshots,
        }
    }
}

pub(crate) struct RepositorySetParts {
    pub(crate) api_keys: Arc<dyn crate::ApiKeyRepository>,
    pub(crate) ci_ingest: Arc<dyn crate::CiIngestRepository>,
    pub(crate) roles: Arc<dyn RoleRepository>,
    pub(crate) users: Arc<dyn UserRepository>,
    pub(crate) zones: Arc<dyn ZoneRepository>,
    pub(crate) rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::{
        ChangePasswordOutcome, CreateUserOutcome, DeleteRoleOutcome, DeleteUserOutcome,
        DeleteZoneOutcome, EncodedPasswordHash, NewRole, NewUser, NewZone, PageRequest,
        PersistenceError, RecordSuccessfulLoginOutcome, RoleDetails, RolePatch,
        SetUserRolesOutcome, TenantId, UpdateRoleOutcome, UserCredentials, UserDetails, UserPage,
        Zone, ZonePatch,
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

    struct FakeUserRepository;

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
    impl UserRepository for FakeUserRepository {
        async fn list(
            &self,
            _tenant: &TenantId,
            _page: PageRequest,
        ) -> Result<UserPage, PersistenceError> {
            Ok(UserPage::new(Vec::new(), 0))
        }

        async fn create(
            &self,
            _tenant: &TenantId,
            _user: NewUser,
        ) -> Result<CreateUserOutcome, PersistenceError> {
            Err(PersistenceError::Internal("not used by this test".into()))
        }

        async fn change_password(
            &self,
            _tenant: &TenantId,
            _user_id: i32,
            _password_hash: EncodedPasswordHash,
        ) -> Result<ChangePasswordOutcome, PersistenceError> {
            Ok(ChangePasswordOutcome::NotFound)
        }

        async fn delete(
            &self,
            _tenant: &TenantId,
            _user_id: i32,
        ) -> Result<DeleteUserOutcome, PersistenceError> {
            Ok(DeleteUserOutcome::NotFound)
        }

        async fn set_roles(
            &self,
            _tenant: &TenantId,
            _user_id: i32,
            _role_ids: Vec<i32>,
        ) -> Result<SetUserRolesOutcome, PersistenceError> {
            Ok(SetUserRolesOutcome::UserNotFound)
        }

        async fn find_credentials_by_username(
            &self,
            _tenant: &TenantId,
            _username: &str,
        ) -> Result<Option<UserCredentials>, PersistenceError> {
            Ok(None)
        }

        async fn get_details(
            &self,
            _tenant: &TenantId,
            _user_id: i32,
        ) -> Result<Option<UserDetails>, PersistenceError> {
            Ok(None)
        }

        async fn record_successful_login(
            &self,
            _tenant: &TenantId,
            _user_id: i32,
            _logged_in_at: chrono::DateTime<chrono::Utc>,
        ) -> Result<RecordSuccessfulLoginOutcome, PersistenceError> {
            Ok(RecordSuccessfulLoginOutcome::NotFound)
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
        let users = Arc::new(FakeUserRepository);

        let repositories = RepositorySet::new(RepositorySetInput {
            api_keys: Arc::new(crate::api_keys::tests::RecordingRepository::default()),
            ci_ingest: Arc::new(crate::api_keys::tests::RecordingRepository::default()),
            roles: roles.clone(),
            users: users.clone(),
            zones: zones.clone(),
            rule_zone_snapshots: rule_zone_snapshots.clone(),
        });
        let parts = repositories.into_parts();

        assert!(Arc::ptr_eq(
            &parts.roles,
            &(roles as Arc<dyn RoleRepository>)
        ));
        assert!(Arc::ptr_eq(
            &parts.users,
            &(users as Arc<dyn UserRepository>)
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
