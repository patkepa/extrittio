use std::sync::Arc;

use crate::application::require_permission;
use crate::{ADMIN_ROLE, OWNER_ROLE};
use crate::{
    Actor, ApplicationError, ChangePasswordOutcome, Clock, CreateUserOutcome, DeleteUserOutcome,
    EncodedPasswordHash, NewUser, PageRequest, PasswordHasher, Permission, Role,
    SetUserRolesOutcome, TenantContext, TenantId, UserCredentials, UserDetails, UserPage,
    UserRepository,
};

pub const MIN_PASSWORD_LEN: usize = 12;

/// Plaintext input for user creation.
///
/// Deliberately does not implement `Debug`, `Clone`, or serialization so a
/// password cannot be copied or logged accidentally through this type.
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub role_ids: Option<Vec<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub id: i32,
    pub tenant_id: TenantId,
    pub username: String,
    pub role: String,
    pub roles: Vec<Role>,
    pub permissions: Vec<String>,
    pub permission_version: i32,
    pub auth_epoch: crate::UserAuthEpoch,
}

#[derive(Clone)]
pub struct UserApplication {
    repository: Arc<dyn UserRepository>,
    password_hasher: Arc<dyn PasswordHasher>,
    clock: Arc<dyn Clock>,
}

impl UserApplication {
    #[must_use]
    pub fn new(
        repository: Arc<dyn UserRepository>,
        password_hasher: Arc<dyn PasswordHasher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            password_hasher,
            clock,
        }
    }

    pub async fn list(
        &self,
        context: &TenantContext,
        page: PageRequest,
    ) -> Result<UserPage, ApplicationError> {
        require_permission(context, Permission::ReadUsers)?;
        Ok(self.repository.list(context.tenant_id(), page).await?)
    }

    pub async fn create(
        &self,
        context: &TenantContext,
        input: CreateUser,
    ) -> Result<UserDetails, ApplicationError> {
        require_permission(context, Permission::ManageUsers)?;
        let username = input.username.trim().to_string();
        if username.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "Username must not be empty".into(),
            ));
        }
        validate_password(&input.password)?;
        if input.role_ids.as_deref().is_some_and(<[i32]>::is_empty) {
            return Err(ApplicationError::InvalidInput(
                "At least one role is required".into(),
            ));
        }

        let role_ids = input.role_ids.map(normalize_role_ids);
        let password_hash = self
            .password_hasher
            .hash(input.password)
            .await
            .map_err(|error| ApplicationError::Authentication(error.to_string()))?;
        let outcome = self
            .repository
            .create(
                context.tenant_id(),
                NewUser {
                    username: username.clone(),
                    password_hash,
                    role_ids,
                },
            )
            .await
            .map_err(|error| map_user_write_error(error, &username))?;
        match outcome {
            CreateUserOutcome::Created(user) => Ok(user),
            CreateUserOutcome::RolesNotFound => Err(ApplicationError::NotFound(
                "One or more roles were not found".into(),
            )),
        }
    }

    pub async fn change_password(
        &self,
        context: &TenantContext,
        user_id: i32,
        new_password: String,
    ) -> Result<(), ApplicationError> {
        require_permission(context, Permission::ManageUsers)?;
        validate_password(&new_password)?;
        let password_hash = self
            .password_hasher
            .hash(new_password)
            .await
            .map_err(|error| ApplicationError::Authentication(error.to_string()))?;
        match self
            .repository
            .change_password(context.tenant_id(), user_id, password_hash)
            .await?
        {
            ChangePasswordOutcome::PasswordChanged => Ok(()),
            ChangePasswordOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "User {user_id} not found"
            ))),
        }
    }

    pub async fn delete(
        &self,
        context: &TenantContext,
        user_id: i32,
    ) -> Result<(), ApplicationError> {
        require_permission(context, Permission::ManageUsers)?;
        match self.repository.delete(context.tenant_id(), user_id).await? {
            DeleteUserOutcome::Deleted => Ok(()),
            DeleteUserOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "User {user_id} not found"
            ))),
            DeleteUserOutcome::WouldDeleteLastOwner => Err(ApplicationError::Conflict(
                "Cannot delete the last owner from the tenant".into(),
            )),
        }
    }

    pub async fn set_roles(
        &self,
        context: &TenantContext,
        user_id: i32,
        role_ids: Vec<i32>,
    ) -> Result<UserDetails, ApplicationError> {
        require_permission(context, Permission::ManageUsers)?;
        if role_ids.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "At least one role is required".into(),
            ));
        }
        match self
            .repository
            .set_roles(context.tenant_id(), user_id, normalize_role_ids(role_ids))
            .await?
        {
            SetUserRolesOutcome::Updated(user) => Ok(user),
            SetUserRolesOutcome::UserNotFound => Err(ApplicationError::NotFound(format!(
                "User {user_id} not found"
            ))),
            SetUserRolesOutcome::RolesNotFound => Err(ApplicationError::NotFound(
                "One or more roles were not found".into(),
            )),
            SetUserRolesOutcome::WouldRemoveLastOwner => Err(ApplicationError::Conflict(
                "Cannot remove the last owner from the tenant".into(),
            )),
        }
    }

    /// Authenticate an exact username within an explicitly selected tenant.
    /// JWT issuance and default-tenant login compatibility remain host-owned.
    pub async fn authenticate(
        &self,
        tenant: &TenantId,
        username: &str,
        password: String,
    ) -> Result<AuthenticatedUser, ApplicationError> {
        let credentials = self
            .repository
            .find_credentials_by_username(tenant, username)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        let UserCredentials {
            details,
            password_hash,
        } = credentials;
        if !details.user.is_active {
            return Err(ApplicationError::Unauthorized);
        }
        if !self.verify_password(password, password_hash).await? {
            return Err(ApplicationError::Unauthorized);
        }

        // Preserve the existing race behavior: once credentials verify, a
        // concurrent delete that makes this best-effort timestamp update miss
        // does not change the authentication result.
        let _outcome = self
            .repository
            .record_successful_login(tenant, details.user.id, self.clock.now())
            .await?;
        Ok(authenticated_user_from_details(details))
    }

    /// Resolve current persisted authorization for an already validated host
    /// credential without admitting JWT types into core.
    pub async fn resolve_session(
        &self,
        tenant: &TenantId,
        user_id: i32,
        permission_version: i32,
        auth_epoch: &str,
    ) -> Result<AuthenticatedUser, ApplicationError> {
        let details = self
            .repository
            .get_details(tenant, user_id)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if !matches_identity(&details, tenant, user_id)
            || !details.user.is_active
            || details.user.permission_version != permission_version
            || details.user.auth_epoch.as_str() != auth_epoch
        {
            return Err(ApplicationError::Unauthorized);
        }
        Ok(authenticated_user_from_details(details))
    }

    pub async fn current_user(
        &self,
        context: &TenantContext,
    ) -> Result<AuthenticatedUser, ApplicationError> {
        let Actor::User { id: user_id, .. } = context.actor() else {
            return Err(ApplicationError::Unauthorized);
        };
        let details = self
            .repository
            .get_details(context.tenant_id(), *user_id)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if !matches_identity(&details, context.tenant_id(), *user_id) || !details.user.is_active {
            return Err(ApplicationError::Unauthorized);
        }
        Ok(authenticated_user_from_details(details))
    }

    async fn verify_password(
        &self,
        password: String,
        password_hash: EncodedPasswordHash,
    ) -> Result<bool, ApplicationError> {
        self.password_hasher
            .verify(password, password_hash)
            .await
            .map_err(|error| {
                ApplicationError::Internal(format!("password verification failed: {error}"))
            })
    }
}

#[must_use]
pub fn authenticated_user_from_details(details: UserDetails) -> AuthenticatedUser {
    let role = primary_role_name(&details.roles)
        .unwrap_or(&details.user.role)
        .to_string();
    AuthenticatedUser {
        id: details.user.id,
        tenant_id: details.user.tenant_id,
        username: details.user.username,
        role,
        roles: details.roles,
        permissions: details.permissions,
        permission_version: details.user.permission_version,
        auth_epoch: details.user.auth_epoch,
    }
}

#[must_use]
pub fn primary_role_name(roles: &[Role]) -> Option<&str> {
    roles
        .iter()
        .find(|role| role.name == OWNER_ROLE)
        .or_else(|| roles.iter().find(|role| role.name == ADMIN_ROLE))
        .or_else(|| roles.first())
        .map(|role| role.name.as_str())
}

pub fn validate_password(password: &str) -> Result<(), ApplicationError> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(ApplicationError::InvalidInput(format!(
            "Password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }

    let has_lower = password
        .chars()
        .any(|character| character.is_ascii_lowercase());
    let has_upper = password
        .chars()
        .any(|character| character.is_ascii_uppercase());
    let has_digit = password.chars().any(|character| character.is_ascii_digit());
    let has_symbol = password
        .chars()
        .any(|character| !character.is_ascii_alphanumeric());

    if !(has_lower && has_upper && has_digit && has_symbol) {
        return Err(ApplicationError::InvalidInput(
            "Password must include lowercase, uppercase, number, and symbol characters".into(),
        ));
    }
    Ok(())
}

fn normalize_role_ids(mut role_ids: Vec<i32>) -> Vec<i32> {
    role_ids.sort_unstable();
    role_ids.dedup();
    role_ids
}

fn matches_identity(details: &UserDetails, tenant: &TenantId, user_id: i32) -> bool {
    details.user.id == user_id && &details.user.tenant_id == tenant
}

fn map_user_write_error(error: crate::PersistenceError, username: &str) -> ApplicationError {
    match error {
        crate::PersistenceError::UniqueViolation { .. } => {
            ApplicationError::Conflict(format!("Username '{username}' already exists"))
        }
        other => ApplicationError::Persistence(other),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::{DateTime, TimeZone, Utc};
    use futures::executor::block_on;

    use super::*;
    use crate::{
        ConstraintName, Page, PasswordHasherError, PermissionSet, PersistenceError,
        RecordSuccessfulLoginOutcome, User,
    };

    struct RepositoryState {
        list_page: UserPage,
        create_outcome: CreateUserOutcome,
        create_unique_violation: bool,
        change_password_outcome: ChangePasswordOutcome,
        delete_outcome: DeleteUserOutcome,
        set_roles_outcome: SetUserRolesOutcome,
        credentials: Option<UserCredentials>,
        details: Option<UserDetails>,
        record_login_outcome: RecordSuccessfulLoginOutcome,
        list_inputs: Vec<(TenantId, PageRequest)>,
        create_inputs: Vec<(TenantId, NewUser)>,
        change_password_inputs: Vec<(TenantId, i32, EncodedPasswordHash)>,
        delete_inputs: Vec<(TenantId, i32)>,
        set_roles_inputs: Vec<(TenantId, i32, Vec<i32>)>,
        credential_inputs: Vec<(TenantId, String)>,
        details_inputs: Vec<(TenantId, i32)>,
        record_login_inputs: Vec<(TenantId, i32, DateTime<Utc>)>,
    }

    struct RecordingRepository {
        state: Mutex<RepositoryState>,
    }

    impl RecordingRepository {
        fn new(details: UserDetails) -> Self {
            Self {
                state: Mutex::new(RepositoryState {
                    list_page: Page::new(vec![details.clone()], 1),
                    create_outcome: CreateUserOutcome::Created(details.clone()),
                    create_unique_violation: false,
                    change_password_outcome: ChangePasswordOutcome::PasswordChanged,
                    delete_outcome: DeleteUserOutcome::Deleted,
                    set_roles_outcome: SetUserRolesOutcome::Updated(details.clone()),
                    credentials: Some(UserCredentials {
                        details: details.clone(),
                        password_hash: EncodedPasswordHash::new("stored-hash"),
                    }),
                    details: Some(details),
                    record_login_outcome: RecordSuccessfulLoginOutcome::LoginRecorded,
                    list_inputs: Vec::new(),
                    create_inputs: Vec::new(),
                    change_password_inputs: Vec::new(),
                    delete_inputs: Vec::new(),
                    set_roles_inputs: Vec::new(),
                    credential_inputs: Vec::new(),
                    details_inputs: Vec::new(),
                    record_login_inputs: Vec::new(),
                }),
            }
        }
    }

    #[async_trait]
    impl UserRepository for RecordingRepository {
        async fn list(
            &self,
            tenant: &TenantId,
            page: PageRequest,
        ) -> Result<UserPage, PersistenceError> {
            let mut state = self.state.lock().unwrap();
            state.list_inputs.push((tenant.clone(), page));
            Ok(state.list_page.clone())
        }

        async fn create(
            &self,
            tenant: &TenantId,
            user: NewUser,
        ) -> Result<CreateUserOutcome, PersistenceError> {
            let mut state = self.state.lock().unwrap();
            state.create_inputs.push((tenant.clone(), user));
            if state.create_unique_violation {
                return Err(PersistenceError::UniqueViolation {
                    constraint: ConstraintName::new("users_tenant_username_key"),
                });
            }
            Ok(state.create_outcome.clone())
        }

        async fn change_password(
            &self,
            tenant: &TenantId,
            user_id: i32,
            password_hash: EncodedPasswordHash,
        ) -> Result<ChangePasswordOutcome, PersistenceError> {
            let mut state = self.state.lock().unwrap();
            state
                .change_password_inputs
                .push((tenant.clone(), user_id, password_hash));
            Ok(state.change_password_outcome)
        }

        async fn delete(
            &self,
            tenant: &TenantId,
            user_id: i32,
        ) -> Result<DeleteUserOutcome, PersistenceError> {
            let mut state = self.state.lock().unwrap();
            state.delete_inputs.push((tenant.clone(), user_id));
            Ok(state.delete_outcome)
        }

        async fn set_roles(
            &self,
            tenant: &TenantId,
            user_id: i32,
            role_ids: Vec<i32>,
        ) -> Result<SetUserRolesOutcome, PersistenceError> {
            let mut state = self.state.lock().unwrap();
            state
                .set_roles_inputs
                .push((tenant.clone(), user_id, role_ids));
            Ok(state.set_roles_outcome.clone())
        }

        async fn find_credentials_by_username(
            &self,
            tenant: &TenantId,
            username: &str,
        ) -> Result<Option<UserCredentials>, PersistenceError> {
            let mut state = self.state.lock().unwrap();
            state
                .credential_inputs
                .push((tenant.clone(), username.to_string()));
            Ok(state.credentials.clone())
        }

        async fn get_details(
            &self,
            tenant: &TenantId,
            user_id: i32,
        ) -> Result<Option<UserDetails>, PersistenceError> {
            let mut state = self.state.lock().unwrap();
            state.details_inputs.push((tenant.clone(), user_id));
            Ok(state.details.clone())
        }

        async fn record_successful_login(
            &self,
            tenant: &TenantId,
            user_id: i32,
            logged_in_at: DateTime<Utc>,
        ) -> Result<RecordSuccessfulLoginOutcome, PersistenceError> {
            let mut state = self.state.lock().unwrap();
            state
                .record_login_inputs
                .push((tenant.clone(), user_id, logged_in_at));
            Ok(state.record_login_outcome)
        }
    }

    struct HasherState {
        hash_result: Result<EncodedPasswordHash, PasswordHasherError>,
        verify_result: Result<bool, PasswordHasherError>,
        hash_inputs: Vec<String>,
        verify_inputs: Vec<(String, EncodedPasswordHash)>,
    }

    struct RecordingPasswordHasher {
        state: Mutex<HasherState>,
    }

    impl RecordingPasswordHasher {
        fn new() -> Self {
            Self {
                state: Mutex::new(HasherState {
                    hash_result: Ok(EncodedPasswordHash::new("new-hash")),
                    verify_result: Ok(true),
                    hash_inputs: Vec::new(),
                    verify_inputs: Vec::new(),
                }),
            }
        }
    }

    #[async_trait]
    impl PasswordHasher for RecordingPasswordHasher {
        async fn hash(
            &self,
            plaintext: String,
        ) -> Result<EncodedPasswordHash, PasswordHasherError> {
            let mut state = self.state.lock().unwrap();
            state.hash_inputs.push(plaintext);
            state.hash_result.clone()
        }

        async fn verify(
            &self,
            plaintext: String,
            password_hash: EncodedPasswordHash,
        ) -> Result<bool, PasswordHasherError> {
            let mut state = self.state.lock().unwrap();
            state.verify_inputs.push((plaintext, password_hash));
            state.verify_result.clone()
        }
    }

    struct FixedClock {
        now: DateTime<Utc>,
        calls: Mutex<usize>,
    }

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            *self.calls.lock().unwrap() += 1;
            self.now
        }
    }

    fn tenant(value: &str) -> TenantId {
        TenantId::new(value).unwrap()
    }

    fn role(tenant_id: &TenantId, id: i32, name: &str) -> Role {
        let time = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        Role {
            id,
            tenant_id: tenant_id.clone(),
            name: name.to_string(),
            description: None,
            is_system: matches!(name, OWNER_ROLE | ADMIN_ROLE),
            created_at: time,
            updated_at: time,
        }
    }

    fn details(tenant_id: &TenantId, active: bool, roles: Vec<Role>) -> UserDetails {
        UserDetails {
            user: User {
                id: 7,
                tenant_id: tenant_id.clone(),
                username: "alice".into(),
                role: "legacy-role".into(),
                created_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
                is_active: active,
                permission_version: 4,
                auth_epoch: crate::UserAuthEpoch::new("auth-epoch-7"),
                last_login_at: None,
            },
            roles,
            permissions: vec!["users.read".into(), "zones.read".into()],
        }
    }

    fn context(tenant_id: &TenantId, permission_keys: &[&str]) -> TenantContext {
        TenantContext::new(
            tenant_id.clone(),
            Actor::User {
                id: 7,
                username: "alice".into(),
                role: "admin".into(),
            },
            PermissionSet::from_keys(permission_keys),
        )
    }

    fn application(
        tenant_id: &TenantId,
    ) -> (
        UserApplication,
        Arc<RecordingRepository>,
        Arc<RecordingPasswordHasher>,
        Arc<FixedClock>,
    ) {
        let user_details = details(tenant_id, true, vec![role(tenant_id, 3, "viewer")]);
        let repository = Arc::new(RecordingRepository::new(user_details));
        let password_hasher = Arc::new(RecordingPasswordHasher::new());
        let clock = Arc::new(FixedClock {
            now: Utc.timestamp_opt(1_800_000_000, 123_000_000).unwrap(),
            calls: Mutex::new(0),
        });
        let application =
            UserApplication::new(repository.clone(), password_hasher.clone(), clock.clone());
        (application, repository, password_hasher, clock)
    }

    fn assert_invalid_input(error: ApplicationError, expected: &str) {
        match error {
            ApplicationError::InvalidInput(message) => assert_eq!(message, expected),
            other => panic!("expected invalid input, got {other:?}"),
        }
    }

    fn assert_unauthorized(error: ApplicationError) {
        assert!(matches!(error, ApplicationError::Unauthorized), "{error:?}");
    }

    #[test]
    fn password_policy_uses_utf8_byte_length_and_ascii_categories() {
        // Eight characters, but twelve UTF-8 bytes, preserving the existing
        // byte-length policy. Non-ASCII characters also satisfy "symbol".
        assert!(validate_password("Aa1!éééé").is_ok());
        assert_invalid_input(
            validate_password("Aa1!short").unwrap_err(),
            "Password must be at least 12 characters",
        );
        assert_invalid_input(
            validate_password("abcdefghijkl").unwrap_err(),
            "Password must include lowercase, uppercase, number, and symbol characters",
        );
    }

    #[test]
    fn create_authorizes_before_validation_and_never_hashes_rejected_input() {
        let tenant_id = tenant("tenant-a");
        let (application, repository, hasher, _) = application(&tenant_id);
        let error = block_on(application.create(
            &context(&tenant_id, &[]),
            CreateUser {
                username: " ".into(),
                password: "weak".into(),
                role_ids: Some(Vec::new()),
            },
        ))
        .unwrap_err();

        match error {
            ApplicationError::Forbidden(message) => {
                assert_eq!(message, "Missing permission 'users.manage'")
            }
            other => panic!("expected forbidden, got {other:?}"),
        }
        assert!(hasher.state.lock().unwrap().hash_inputs.is_empty());
        assert!(repository.state.lock().unwrap().create_inputs.is_empty());
    }

    #[test]
    fn create_validates_in_legacy_order_and_normalizes_the_repository_input() {
        let tenant_id = tenant("tenant-a");
        let ctx = context(&tenant_id, &["users.manage"]);
        let (application, repository, hasher, _) = application(&tenant_id);

        assert_invalid_input(
            block_on(application.create(
                &ctx,
                CreateUser {
                    username: "  ".into(),
                    password: "CorrectHorse1!".into(),
                    role_ids: None,
                },
            ))
            .unwrap_err(),
            "Username must not be empty",
        );
        assert_invalid_input(
            block_on(application.create(
                &ctx,
                CreateUser {
                    username: "alice".into(),
                    password: "weak".into(),
                    role_ids: Some(Vec::new()),
                },
            ))
            .unwrap_err(),
            "Password must be at least 12 characters",
        );
        assert_invalid_input(
            block_on(application.create(
                &ctx,
                CreateUser {
                    username: "alice".into(),
                    password: "CorrectHorse1!".into(),
                    role_ids: Some(Vec::new()),
                },
            ))
            .unwrap_err(),
            "At least one role is required",
        );

        let created = block_on(application.create(
            &ctx,
            CreateUser {
                username: "  alice  ".into(),
                password: "CorrectHorse1!".into(),
                role_ids: Some(vec![9, 2, 9, 4]),
            },
        ))
        .unwrap();
        assert_eq!(created.user.username, "alice");

        assert_eq!(
            hasher.state.lock().unwrap().hash_inputs,
            vec!["CorrectHorse1!".to_string()]
        );
        let state = repository.state.lock().unwrap();
        let (actual_tenant, input) = state.create_inputs.last().unwrap();
        assert_eq!(actual_tenant, &tenant_id);
        assert_eq!(input.username, "alice");
        assert_eq!(input.password_hash.as_str(), "new-hash");
        assert_eq!(input.role_ids, Some(vec![2, 4, 9]));
    }

    #[test]
    fn create_maps_role_duplicate_and_hash_failures_to_stable_errors() {
        let tenant_id = tenant("tenant-a");
        let ctx = context(&tenant_id, &["users.manage"]);
        let (application, repository, hasher, _) = application(&tenant_id);

        repository.state.lock().unwrap().create_outcome = CreateUserOutcome::RolesNotFound;
        let error = block_on(application.create(
            &ctx,
            CreateUser {
                username: "alice".into(),
                password: "CorrectHorse1!".into(),
                role_ids: Some(vec![99]),
            },
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::NotFound(ref message)
                if message == "One or more roles were not found"
        ));

        repository.state.lock().unwrap().create_unique_violation = true;
        let error = block_on(application.create(
            &ctx,
            CreateUser {
                username: " alice ".into(),
                password: "CorrectHorse1!".into(),
                role_ids: None,
            },
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::Conflict(ref message)
                if message == "Username 'alice' already exists"
        ));

        repository.state.lock().unwrap().create_unique_violation = false;
        hasher.state.lock().unwrap().hash_result =
            Err(PasswordHasherError::new("hash worker unavailable"));
        let call_count = repository.state.lock().unwrap().create_inputs.len();
        let error = block_on(application.create(
            &ctx,
            CreateUser {
                username: "bob".into(),
                password: "CorrectHorse1!".into(),
                role_ids: None,
            },
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::Authentication(ref message)
                if message == "hash worker unavailable"
        ));
        assert_eq!(
            repository.state.lock().unwrap().create_inputs.len(),
            call_count
        );
    }

    #[test]
    fn list_accepts_manage_as_read_and_preserves_tenant_and_i64_page() {
        let tenant_id = tenant("tenant-a");
        let (application, repository, _, _) = application(&tenant_id);
        let page = PageRequest::new(i64::MAX, i64::MAX).unwrap();

        let result =
            block_on(application.list(&context(&tenant_id, &["users.manage"]), page)).unwrap();

        assert_eq!(result.total, 1);
        assert_eq!(
            repository.state.lock().unwrap().list_inputs,
            vec![(tenant_id, page)]
        );
    }

    #[test]
    fn password_change_maps_hash_and_typed_not_found_failures() {
        let tenant_id = tenant("tenant-a");
        let ctx = context(&tenant_id, &["users.manage"]);
        let (application, repository, hasher, _) = application(&tenant_id);
        repository.state.lock().unwrap().change_password_outcome = ChangePasswordOutcome::NotFound;

        let error =
            block_on(application.change_password(&ctx, 42, "CorrectHorse1!".into())).unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::NotFound(ref message) if message == "User 42 not found"
        ));
        let state = repository.state.lock().unwrap();
        assert_eq!(state.change_password_inputs.len(), 1);
        assert_eq!(state.change_password_inputs[0].0, tenant_id);
        assert_eq!(state.change_password_inputs[0].1, 42);
        assert_eq!(state.change_password_inputs[0].2.as_str(), "new-hash");
        drop(state);

        hasher.state.lock().unwrap().hash_result = Err(PasswordHasherError::new("hash failed"));
        let error =
            block_on(application.change_password(&ctx, 42, "AnotherHorse1!".into())).unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::Authentication(ref message) if message == "hash failed"
        ));
        assert_eq!(
            repository
                .state
                .lock()
                .unwrap()
                .change_password_inputs
                .len(),
            1
        );
    }

    #[test]
    fn delete_maps_not_found_and_last_owner_outcomes() {
        let tenant_id = tenant("tenant-a");
        let ctx = context(&tenant_id, &["users.manage"]);
        let (application, repository, _, _) = application(&tenant_id);

        repository.state.lock().unwrap().delete_outcome = DeleteUserOutcome::NotFound;
        let error = block_on(application.delete(&ctx, 11)).unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::NotFound(ref message) if message == "User 11 not found"
        ));

        repository.state.lock().unwrap().delete_outcome = DeleteUserOutcome::WouldDeleteLastOwner;
        let error = block_on(application.delete(&ctx, 11)).unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::Conflict(ref message)
                if message == "Cannot delete the last owner from the tenant"
        ));
    }

    #[test]
    fn set_roles_normalizes_ids_and_maps_all_typed_failures() {
        let tenant_id = tenant("tenant-a");
        let ctx = context(&tenant_id, &["users.manage"]);
        let (application, repository, _, _) = application(&tenant_id);

        assert_invalid_input(
            block_on(application.set_roles(&ctx, 7, Vec::new())).unwrap_err(),
            "At least one role is required",
        );
        repository.state.lock().unwrap().set_roles_outcome = SetUserRolesOutcome::UserNotFound;
        let error = block_on(application.set_roles(&ctx, 17, vec![3, 1, 3])).unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::NotFound(ref message) if message == "User 17 not found"
        ));
        assert_eq!(
            repository.state.lock().unwrap().set_roles_inputs[0],
            (tenant_id.clone(), 17, vec![1, 3])
        );

        repository.state.lock().unwrap().set_roles_outcome = SetUserRolesOutcome::RolesNotFound;
        let error = block_on(application.set_roles(&ctx, 17, vec![99])).unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::NotFound(ref message)
                if message == "One or more roles were not found"
        ));

        repository.state.lock().unwrap().set_roles_outcome =
            SetUserRolesOutcome::WouldRemoveLastOwner;
        let error = block_on(application.set_roles(&ctx, 17, vec![1])).unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::Conflict(ref message)
                if message == "Cannot remove the last owner from the tenant"
        ));
    }

    #[test]
    fn authentication_fails_closed_for_missing_inactive_wrong_and_malformed_credentials() {
        let tenant_id = tenant("tenant-a");
        let (application, repository, hasher, clock) = application(&tenant_id);

        repository.state.lock().unwrap().credentials = None;
        assert_unauthorized(
            block_on(application.authenticate(&tenant_id, "alice", "secret".into())).unwrap_err(),
        );
        assert!(hasher.state.lock().unwrap().verify_inputs.is_empty());

        repository.state.lock().unwrap().credentials = Some(UserCredentials {
            details: details(&tenant_id, false, Vec::new()),
            password_hash: EncodedPasswordHash::new("stored-hash"),
        });
        assert_unauthorized(
            block_on(application.authenticate(&tenant_id, "alice", "secret".into())).unwrap_err(),
        );
        assert!(hasher.state.lock().unwrap().verify_inputs.is_empty());

        repository.state.lock().unwrap().credentials = Some(UserCredentials {
            details: details(&tenant_id, true, Vec::new()),
            password_hash: EncodedPasswordHash::new("stored-hash"),
        });
        hasher.state.lock().unwrap().verify_result = Ok(false);
        assert_unauthorized(
            block_on(application.authenticate(&tenant_id, "alice", "wrong".into())).unwrap_err(),
        );

        repository.state.lock().unwrap().credentials = Some(UserCredentials {
            details: details(&tenant_id, true, Vec::new()),
            password_hash: EncodedPasswordHash::new("malformed-encoding"),
        });
        assert_unauthorized(
            block_on(application.authenticate(&tenant_id, "alice", "secret".into())).unwrap_err(),
        );
        assert_eq!(*clock.calls.lock().unwrap(), 0);
        assert!(
            repository
                .state
                .lock()
                .unwrap()
                .record_login_inputs
                .is_empty()
        );
    }

    #[test]
    fn authentication_uses_exact_username_and_deterministic_clock() {
        let tenant_id = tenant("tenant-a");
        let (application, repository, hasher, clock) = application(&tenant_id);
        repository.state.lock().unwrap().record_login_outcome =
            RecordSuccessfulLoginOutcome::NotFound;

        let authenticated =
            block_on(application.authenticate(&tenant_id, " Alice ", "plaintext-secret".into()))
                .unwrap();

        assert_eq!(authenticated.id, 7);
        assert_eq!(authenticated.tenant_id, tenant_id);
        assert_eq!(authenticated.role, "viewer");
        let hasher_state = hasher.state.lock().unwrap();
        assert_eq!(hasher_state.verify_inputs.len(), 1);
        assert_eq!(hasher_state.verify_inputs[0].0, "plaintext-secret");
        assert_eq!(hasher_state.verify_inputs[0].1.as_str(), "stored-hash");
        drop(hasher_state);
        let state = repository.state.lock().unwrap();
        assert_eq!(
            state.credential_inputs,
            vec![(tenant_id.clone(), " Alice ".to_string())]
        );
        assert_eq!(state.record_login_inputs, vec![(tenant_id, 7, clock.now)]);
        assert_eq!(*clock.calls.lock().unwrap(), 1);
    }

    #[test]
    fn password_verifier_failures_are_internal_and_do_not_record_login() {
        let tenant_id = tenant("tenant-a");
        let (application, repository, hasher, _) = application(&tenant_id);
        hasher.state.lock().unwrap().verify_result =
            Err(PasswordHasherError::new("worker panicked"));

        let error =
            block_on(application.authenticate(&tenant_id, "alice", "secret".into())).unwrap_err();
        assert!(matches!(
            error,
            ApplicationError::Internal(ref message)
                if message == "password verification failed: worker panicked"
        ));
        assert!(
            repository
                .state
                .lock()
                .unwrap()
                .record_login_inputs
                .is_empty()
        );
    }

    #[test]
    fn session_resolution_checks_identity_activity_and_permission_version() {
        let tenant_id = tenant("tenant-a");
        let (application, repository, _, _) = application(&tenant_id);

        repository.state.lock().unwrap().details = None;
        assert_unauthorized(
            block_on(application.resolve_session(&tenant_id, 7, 4, "auth-epoch-7")).unwrap_err(),
        );

        repository.state.lock().unwrap().details = Some(details(&tenant_id, false, Vec::new()));
        assert_unauthorized(
            block_on(application.resolve_session(&tenant_id, 7, 4, "auth-epoch-7")).unwrap_err(),
        );

        repository.state.lock().unwrap().details = Some(details(&tenant_id, true, Vec::new()));
        assert_unauthorized(
            block_on(application.resolve_session(&tenant_id, 7, 3, "auth-epoch-7")).unwrap_err(),
        );

        // A replacement principal with the same tenant, numeric ID, username,
        // and permission version must not revive the deleted user's session.
        assert_unauthorized(
            block_on(application.resolve_session(&tenant_id, 7, 4, "deleted-user-epoch"))
                .unwrap_err(),
        );

        repository.state.lock().unwrap().details =
            Some(details(&tenant("other-tenant"), true, Vec::new()));
        assert_unauthorized(
            block_on(application.resolve_session(&tenant_id, 7, 4, "auth-epoch-7")).unwrap_err(),
        );

        let mut wrong_id = details(&tenant_id, true, Vec::new());
        wrong_id.user.id = 8;
        repository.state.lock().unwrap().details = Some(wrong_id);
        assert_unauthorized(
            block_on(application.resolve_session(&tenant_id, 7, 4, "auth-epoch-7")).unwrap_err(),
        );

        let expected = details(&tenant_id, true, vec![role(&tenant_id, 1, ADMIN_ROLE)]);
        repository.state.lock().unwrap().details = Some(expected);
        let resolved =
            block_on(application.resolve_session(&tenant_id, 7, 4, "auth-epoch-7")).unwrap();
        assert_eq!(resolved.role, ADMIN_ROLE);
    }

    #[test]
    fn current_user_requires_a_live_matching_user_actor() {
        let tenant_id = tenant("tenant-a");
        let (application, repository, _, _) = application(&tenant_id);
        let api_context = TenantContext::new(
            tenant_id.clone(),
            Actor::ApiKey { id: "key-1".into() },
            PermissionSet::all(),
        );
        assert_unauthorized(block_on(application.current_user(&api_context)).unwrap_err());
        assert!(repository.state.lock().unwrap().details_inputs.is_empty());

        let user_context = context(&tenant_id, &[]);
        repository.state.lock().unwrap().details = None;
        assert_unauthorized(block_on(application.current_user(&user_context)).unwrap_err());
        repository.state.lock().unwrap().details = Some(details(&tenant_id, false, Vec::new()));
        assert_unauthorized(block_on(application.current_user(&user_context)).unwrap_err());

        repository.state.lock().unwrap().details =
            Some(details(&tenant("other-tenant"), true, Vec::new()));
        assert_unauthorized(block_on(application.current_user(&user_context)).unwrap_err());

        repository.state.lock().unwrap().details = Some(details(&tenant_id, true, Vec::new()));
        assert_eq!(
            block_on(application.current_user(&user_context))
                .unwrap()
                .username,
            "alice"
        );
    }

    #[test]
    fn primary_role_priority_is_owner_then_admin_then_first_then_legacy() {
        let tenant_id = tenant("tenant-a");
        let roles = vec![
            role(&tenant_id, 3, "custom"),
            role(&tenant_id, 2, ADMIN_ROLE),
            role(&tenant_id, 1, OWNER_ROLE),
        ];
        assert_eq!(primary_role_name(&roles), Some(OWNER_ROLE));
        assert_eq!(primary_role_name(&roles[..2]), Some(ADMIN_ROLE));
        assert_eq!(primary_role_name(&roles[..1]), Some("custom"));
        assert_eq!(primary_role_name(&[]), None);

        let authenticated = authenticated_user_from_details(details(&tenant_id, true, Vec::new()));
        assert_eq!(authenticated.role, "legacy-role");
    }
}
