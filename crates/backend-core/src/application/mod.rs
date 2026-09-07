mod api_keys;
pub use api_keys::ApiKeyApplication;
mod roles;
mod users;
mod zones;

use std::sync::Arc;

use crate::{
    Clock, PasswordHasher, Permission, RepositorySet, RuleZoneSnapshotRepository, TenantContext,
};

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
    ) -> Self {
        Self {
            password_hasher,
            clock,
            api_key_generator,
        }
    }
}

/// Curated application façade passed to transports.
#[derive(Clone)]
pub struct Application {
    api_keys: ApiKeyApplication,
    roles: RoleApplication,
    users: UserApplication,
    zones: ZoneApplication,
    // Retained for the rules application slice. Keeping the port here ensures
    // `RepositorySet` is consumed by the application instead of becoming a
    // host-level service locator during the incremental migration.
    _rule_zone_snapshots: Arc<dyn RuleZoneSnapshotRepository>,
}

impl Application {
    #[must_use]
    pub fn new(repositories: RepositorySet, dependencies: ApplicationDependencies) -> Self {
        let repositories = repositories.into_parts();
        Self {
            api_keys: ApiKeyApplication::new(repositories.api_keys, dependencies.api_key_generator),
            roles: RoleApplication::new(repositories.roles),
            users: UserApplication::new(
                repositories.users,
                dependencies.password_hasher,
                dependencies.clock,
            ),
            zones: ZoneApplication::new(repositories.zones),
            _rule_zone_snapshots: repositories.rule_zone_snapshots,
        }
    }

    #[must_use]
    pub fn api_keys(&self) -> &ApiKeyApplication {
        &self.api_keys
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

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::{
        ChangePasswordOutcome, CreateUserOutcome, DeleteRoleOutcome, DeleteUserOutcome,
        DeleteZoneOutcome, EncodedPasswordHash, NewRole, NewUser, NewZone, PageRequest,
        PasswordHasherError, PersistenceError, RecordSuccessfulLoginOutcome, RepositorySetInput,
        RoleDetails, RolePatch, RoleRepository, SetUserRolesOutcome, TenantId, UpdateRoleOutcome,
        UserCredentials, UserDetails, UserPage, UserRepository, Zone, ZonePatch, ZoneRepository,
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

    #[async_trait]
    impl UserRepository for FakeRepositories {
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

    struct FakePasswordHasher;

    #[async_trait]
    impl PasswordHasher for FakePasswordHasher {
        async fn hash(
            &self,
            _plaintext: String,
        ) -> Result<EncodedPasswordHash, PasswordHasherError> {
            Ok(EncodedPasswordHash::new("unused"))
        }

        async fn verify(
            &self,
            _plaintext: String,
            _password_hash: EncodedPasswordHash,
        ) -> Result<bool, PasswordHasherError> {
            Ok(false)
        }
    }

    struct FakeClock;

    impl Clock for FakeClock {
        fn now(&self) -> chrono::DateTime<chrono::Utc> {
            chrono::DateTime::UNIX_EPOCH
        }
    }

    #[test]
    fn application_consumes_and_retains_the_complete_repository_set() {
        let repository = Arc::new(FakeRepositories);
        let application = Application::new(
            RepositorySet::new(RepositorySetInput {
                api_keys: Arc::new(crate::api_keys::tests::RecordingRepository::default()),
                roles: repository.clone(),
                users: repository.clone(),
                zones: repository.clone(),
                rule_zone_snapshots: repository.clone(),
            }),
            ApplicationDependencies::new(
                Arc::new(FakePasswordHasher),
                Arc::new(FakeClock),
                Arc::new(crate::api_keys::tests::TestGenerator),
            ),
        );

        // The caller, zone façade, and retained snapshot port each hold one
        // reference. No database runtime or lifecycle handle is required.
        assert_eq!(Arc::strong_count(&repository), 5);
        let _roles = application.roles();
        let _users = application.users();
        let _zones = application.zones();
    }
}
