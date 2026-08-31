use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use extrittio_backend_core::{
    ChangePasswordOutcome, CreateUserOutcome, DeleteRoleOutcome, DeleteUserOutcome,
    EncodedPasswordHash, NewUser, PageRequest, PersistenceError, RecordSuccessfulLoginOutcome,
    Role, RolePatch, RoleRepository, SetUserRolesOutcome, TenantId, UpdateRoleOutcome, UserDetails,
    UserRepository,
};

#[async_trait]
pub trait UserContractHarness: Send + Sync {
    async fn reset_users(&self) -> Result<(), PersistenceError>;
    fn users(&self) -> Arc<dyn UserRepository>;
    fn roles(&self) -> Arc<dyn RoleRepository>;

    /// Raw role fixture hook used for built-ins and bytewise-order sentinels.
    async fn insert_role(
        &self,
        tenant: &TenantId,
        name: &str,
        is_system: bool,
        permissions: &[&str],
    ) -> Result<Role, PersistenceError>;

    async fn set_user_active(
        &self,
        tenant: &TenantId,
        user_id: i32,
        is_active: bool,
    ) -> Result<(), PersistenceError>;

    /// Whether deleting the current maximum user ID is expected to make that
    /// numeric ID available to the next insert. Turso exercises this SQLite
    /// behavior so the contract cannot quietly rely on integer identity.
    fn expects_deleted_user_id_reuse(&self) -> bool {
        false
    }
}

struct RoleFixtures {
    viewer: Role,
    owner: Role,
    admin: Role,
    binary_upper: Role,
    binary_lower: Role,
}

pub async fn assert_contract(harness: &dyn UserContractHarness) {
    assert_crud_and_projection_semantics(harness).await;
    assert_delete_and_recreate_changes_auth_epoch(harness).await;
    assert_concurrent_create_and_role_delete(harness).await;
    assert_concurrent_role_and_user_updates(harness).await;
    assert_inactive_owners_count(harness).await;
    assert_concurrent_owner_delete(harness).await;
    assert_concurrent_owner_removal(harness).await;
    assert_concurrent_owner_delete_and_removal(harness).await;
}

async fn assert_delete_and_recreate_changes_auth_epoch(harness: &dyn UserContractHarness) {
    harness
        .reset_users()
        .await
        .expect("reset user recreation fixture");
    let users = harness.users();
    let tenant = tenant_a();
    let roles = seed_roles(harness, &tenant).await;
    let original = create(
        &*users,
        &tenant,
        "recreated-principal",
        "original-hash",
        Some(vec![roles.viewer.id]),
    )
    .await;
    let original_id = original.user.id;
    let original_epoch = original.user.auth_epoch.clone();
    assert!(!original_epoch.as_str().is_empty());

    assert_eq!(
        users
            .delete(&tenant, original_id)
            .await
            .expect("delete original principal"),
        DeleteUserOutcome::Deleted
    );
    let replacement = create(
        &*users,
        &tenant,
        "recreated-principal",
        "replacement-hash",
        Some(vec![roles.viewer.id]),
    )
    .await;

    if harness.expects_deleted_user_id_reuse() {
        assert_eq!(
            replacement.user.id, original_id,
            "this adapter fixture must exercise numeric user-ID reuse"
        );
    }
    assert_ne!(
        replacement.user.auth_epoch, original_epoch,
        "a replacement principal must never inherit the deleted principal's authentication epoch"
    );
    let credentials = users
        .find_credentials_by_username(&tenant, "recreated-principal")
        .await
        .expect("read replacement credentials")
        .expect("replacement credentials exist");
    assert_eq!(credentials.details, replacement);
    assert_eq!(credentials.password_hash.as_str(), "replacement-hash");
}

async fn assert_inactive_owners_count(harness: &dyn UserContractHarness) {
    harness
        .reset_users()
        .await
        .expect("reset inactive-owner fixture");
    let users = harness.users();
    let tenant = tenant_a();
    let roles = seed_roles(harness, &tenant).await;
    let active_owner = create(
        &*users,
        &tenant,
        "active-owner",
        "active-owner-hash",
        Some(vec![roles.owner.id]),
    )
    .await;
    let inactive_owner = create(
        &*users,
        &tenant,
        "inactive-owner",
        "inactive-owner-hash",
        Some(vec![roles.owner.id]),
    )
    .await;
    harness
        .set_user_active(&tenant, inactive_owner.user.id, false)
        .await
        .expect("deactivate owner fixture");

    assert!(matches!(
        users
            .set_roles(&tenant, active_owner.user.id, vec![roles.viewer.id])
            .await
            .expect("remove owner while inactive owner remains"),
        SetUserRolesOutcome::Updated(_)
    ));
    let inactive_owner = details(&*users, &tenant, inactive_owner.user.id).await;
    assert!(!inactive_owner.user.is_active);
    assert_eq!(
        users
            .delete(&tenant, inactive_owner.user.id)
            .await
            .expect("protect inactive last owner delete"),
        DeleteUserOutcome::WouldDeleteLastOwner
    );
    assert_eq!(
        users
            .set_roles(&tenant, inactive_owner.user.id, vec![roles.viewer.id])
            .await
            .expect("protect inactive last owner removal"),
        SetUserRolesOutcome::WouldRemoveLastOwner
    );
}

async fn assert_concurrent_role_and_user_updates(harness: &dyn UserContractHarness) {
    harness
        .reset_users()
        .await
        .expect("reset concurrent role/user-update fixture");
    let users = harness.users();
    let roles_repository = harness.roles();
    let tenant = tenant_a();
    let roles = seed_roles(harness, &tenant).await;
    let user = create(
        &*users,
        &tenant,
        "concurrent-role-user",
        "concurrent-role-hash",
        Some(vec![roles.binary_lower.id]),
    )
    .await;
    let initial_version = user.user.permission_version;
    let user_id = user.user.id;

    let set_users = users.clone();
    let update_roles = roles_repository;
    let set_tenant = tenant.clone();
    let update_tenant = tenant.clone();
    let role_id = roles.binary_lower.id;
    let (set_result, update_result) = tokio::join!(
        async move {
            set_users
                .set_roles(&set_tenant, user_id, vec![role_id])
                .await
        },
        async move {
            update_roles
                .update(
                    &update_tenant,
                    role_id,
                    RolePatch {
                        name: None,
                        description: None,
                        permissions: Some(vec!["zones.read".into(), "alerts.read".into()]),
                    },
                )
                .await
        },
    );
    assert!(matches!(set_result, Ok(SetUserRolesOutcome::Updated(_))));
    assert!(matches!(update_result, Ok(UpdateRoleOutcome::Updated(_))));
    let final_user = details(&*users, &tenant, user_id).await;
    assert_eq!(
        final_user.user.permission_version,
        initial_version + 2,
        "role replacement and permission update each invalidate exactly once"
    );
    assert_eq!(
        final_user
            .permissions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["alerts.read", "zones.read"]
    );
}

async fn assert_crud_and_projection_semantics(harness: &dyn UserContractHarness) {
    harness
        .reset_users()
        .await
        .expect("reset user contract fixture");
    let users = harness.users();
    let tenant_a = tenant_a();
    let tenant_b = tenant_b();
    let roles_a = seed_roles(harness, &tenant_a).await;
    let roles_b = seed_roles(harness, &tenant_b).await;

    let upper = create(&*users, &tenant_a, "Zulu-user", "hash-upper", None).await;
    let alpha = create(&*users, &tenant_a, "alpha-user", "hash-alpha", None).await;
    let unicode = create(&*users, &tenant_a, "é-user", "hash-unicode", None).await;
    let explicit = create(
        &*users,
        &tenant_a,
        "explicit-user",
        "hash-explicit",
        Some(vec![
            roles_a.binary_lower.id,
            roles_a.binary_upper.id,
            roles_a.binary_lower.id,
        ]),
    )
    .await;
    let owner_priority = create(
        &*users,
        &tenant_a,
        "owner-priority",
        "hash-owner",
        Some(vec![
            roles_a.binary_lower.id,
            roles_a.admin.id,
            roles_a.owner.id,
        ]),
    )
    .await;
    let admin_priority = create(
        &*users,
        &tenant_a,
        "admin-priority",
        "hash-admin",
        Some(vec![roles_a.binary_lower.id, roles_a.admin.id]),
    )
    .await;
    let other_tenant = create(&*users, &tenant_b, "alpha-user", "hash-other-tenant", None).await;

    let mut auth_epochs = HashSet::new();
    for details in [
        &upper,
        &alpha,
        &unicode,
        &explicit,
        &owner_priority,
        &admin_priority,
        &other_tenant,
    ] {
        assert_eq!(details.user.permission_version, 2);
        assert!(!details.user.auth_epoch.as_str().is_empty());
        assert!(
            auth_epochs.insert(details.user.auth_epoch.as_str()),
            "every persisted principal receives a unique authentication epoch"
        );
        assert_microsecond(details.user.created_at);
    }
    assert_eq!(
        role_names(&upper),
        vec!["viewer"],
        "None selects the tenant-local viewer role"
    );
    assert_eq!(upper.user.role, "viewer");
    assert_eq!(role_names(&explicit), vec!["Zulu", "alpha"]);
    assert_eq!(explicit.user.role, "Zulu", "binary-first role is fallback");
    assert_eq!(
        explicit
            .permissions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["Alerts.read", "zones.read", "é.permission"],
        "permissions are distinct and bytewise ordered"
    );
    assert_eq!(owner_priority.user.role, "owner");
    assert_eq!(admin_priority.user.role, "admin");

    let full_page = users
        .list(&tenant_a, PageRequest::new(20, 0).unwrap())
        .await
        .expect("list tenant users");
    assert_eq!(full_page.total, 6);
    assert_eq!(
        usernames(&full_page.records),
        vec![
            "Zulu-user",
            "admin-priority",
            "alpha-user",
            "explicit-user",
            "owner-priority",
            "é-user",
        ]
    );
    let repeated = users
        .list(&tenant_a, PageRequest::new(20, 0).unwrap())
        .await
        .expect("repeat stable user list");
    assert_eq!(repeated, full_page);
    let second_page = users
        .list(&tenant_a, PageRequest::new(2, 1).unwrap())
        .await
        .expect("list paginated users");
    assert_eq!(second_page.total, 6);
    assert_eq!(
        usernames(&second_page.records),
        vec!["admin-priority", "alpha-user"]
    );
    assert_eq!(
        users
            .list(&tenant_b, PageRequest::new(20, 0).unwrap())
            .await
            .expect("list other tenant users")
            .records,
        vec![other_tenant.clone()]
    );

    assert!(
        users
            .get_details(&tenant_b, alpha.user.id)
            .await
            .expect("cross-tenant user details")
            .is_none()
    );
    let other_credentials = users
        .find_credentials_by_username(&tenant_b, "alpha-user")
        .await
        .expect("other-tenant credentials")
        .expect("other-tenant user exists");
    assert_eq!(other_credentials.details.user.id, other_tenant.user.id);
    assert_eq!(
        other_credentials.password_hash.as_str(),
        "hash-other-tenant"
    );
    assert_eq!(
        users
            .change_password(
                &tenant_b,
                alpha.user.id,
                EncodedPasswordHash::new("cross-tenant-hash"),
            )
            .await
            .expect("cross-tenant password change"),
        ChangePasswordOutcome::NotFound
    );
    assert_eq!(
        users
            .record_successful_login(
                &tenant_b,
                alpha.user.id,
                DateTime::from_timestamp_micros(1_700_000_000_000_001).unwrap(),
            )
            .await
            .expect("cross-tenant login update"),
        RecordSuccessfulLoginOutcome::NotFound
    );
    assert_eq!(
        users
            .set_roles(&tenant_b, alpha.user.id, vec![roles_b.viewer.id])
            .await
            .expect("cross-tenant role update"),
        SetUserRolesOutcome::UserNotFound
    );
    assert_eq!(
        users
            .delete(&tenant_b, alpha.user.id)
            .await
            .expect("cross-tenant delete"),
        DeleteUserOutcome::NotFound
    );

    let missing_role_id = i32::MAX;
    assert_eq!(
        users
            .create(
                &tenant_a,
                NewUser {
                    username: "missing-role-create".into(),
                    password_hash: EncodedPasswordHash::new("hash-missing-role"),
                    role_ids: Some(vec![roles_a.viewer.id, missing_role_id]),
                },
            )
            .await
            .expect("create with a missing role"),
        CreateUserOutcome::RolesNotFound
    );
    assert!(
        users
            .find_credentials_by_username(&tenant_a, "missing-role-create")
            .await
            .expect("find rolled-back user")
            .is_none(),
        "a rejected create is atomic"
    );

    let before_missing_set = users
        .get_details(&tenant_a, explicit.user.id)
        .await
        .expect("read before rejected set_roles")
        .expect("explicit user exists");
    assert_eq!(
        users
            .set_roles(
                &tenant_a,
                explicit.user.id,
                vec![roles_a.admin.id, missing_role_id],
            )
            .await
            .expect("set a missing role"),
        SetUserRolesOutcome::RolesNotFound
    );
    assert_eq!(
        users
            .get_details(&tenant_a, explicit.user.id)
            .await
            .expect("read after rejected set_roles")
            .expect("explicit user survives"),
        before_missing_set,
        "a rejected role replacement rolls back assignments and version"
    );
    assert_eq!(
        users
            .set_roles(&tenant_a, i32::MAX, vec![roles_a.viewer.id])
            .await
            .expect("set roles for missing user"),
        SetUserRolesOutcome::UserNotFound
    );

    let duplicate = users
        .create(
            &tenant_a,
            NewUser {
                username: alpha.user.username.clone(),
                password_hash: EncodedPasswordHash::new("duplicate-hash"),
                role_ids: None,
            },
        )
        .await
        .expect_err("tenant-local usernames are unique");
    assert_unique_constraint(&duplicate, "users.tenant_username");

    let initial_version = alpha.user.permission_version;
    for (expected_version, password_hash) in [
        (initial_version + 1, "new-hash"),
        (initial_version + 2, "new-hash"),
    ] {
        assert_eq!(
            users
                .change_password(
                    &tenant_a,
                    alpha.user.id,
                    EncodedPasswordHash::new(password_hash),
                )
                .await
                .expect("change password"),
            ChangePasswordOutcome::PasswordChanged
        );
        assert_eq!(
            details(&*users, &tenant_a, alpha.user.id)
                .await
                .user
                .permission_version,
            expected_version,
            "every successful password write bumps exactly once, including a retry"
        );
    }
    assert_eq!(
        users
            .find_credentials_by_username(&tenant_a, "alpha-user")
            .await
            .expect("find changed credentials")
            .expect("changed user exists")
            .password_hash
            .as_str(),
        "new-hash"
    );
    assert_eq!(
        users
            .change_password(
                &tenant_a,
                i32::MAX,
                EncodedPasswordHash::new("missing-password"),
            )
            .await
            .expect("change missing password"),
        ChangePasswordOutcome::NotFound
    );

    let before_login = details(&*users, &tenant_a, alpha.user.id).await;
    let logged_in_at = DateTime::from_timestamp_micros(1_700_000_000_123_456).unwrap();
    assert_eq!(
        users
            .record_successful_login(&tenant_a, alpha.user.id, logged_in_at)
            .await
            .expect("record successful login"),
        RecordSuccessfulLoginOutcome::LoginRecorded
    );
    let after_login = details(&*users, &tenant_a, alpha.user.id).await;
    assert_eq!(after_login.user.last_login_at, Some(logged_in_at));
    assert_eq!(
        after_login.user.permission_version, before_login.user.permission_version,
        "login bookkeeping does not invalidate credentials"
    );
    assert_microsecond(after_login.user.last_login_at.unwrap());
    let second_logged_in_at = DateTime::from_timestamp_micros(1_700_000_000_654_321).unwrap();
    assert_eq!(
        users
            .record_successful_login(&tenant_a, alpha.user.id, second_logged_in_at)
            .await
            .expect("record a later successful login"),
        RecordSuccessfulLoginOutcome::LoginRecorded
    );
    let after_second_login = details(&*users, &tenant_a, alpha.user.id).await;
    assert_eq!(
        after_second_login.user.last_login_at,
        Some(second_logged_in_at)
    );
    assert_eq!(
        after_second_login.user.permission_version, before_login.user.permission_version,
        "login timestamps are last-write-wins without invalidation"
    );
    assert_eq!(
        users
            .record_successful_login(&tenant_a, i32::MAX, logged_in_at)
            .await
            .expect("record login for missing user"),
        RecordSuccessfulLoginOutcome::NotFound
    );

    let before_same_roles = details(&*users, &tenant_a, explicit.user.id).await;
    for expected_version in [
        before_same_roles.user.permission_version + 1,
        before_same_roles.user.permission_version + 2,
    ] {
        let updated = expect_roles_updated(
            users
                .set_roles(
                    &tenant_a,
                    explicit.user.id,
                    vec![roles_a.admin.id, roles_a.binary_lower.id, roles_a.admin.id],
                )
                .await
                .expect("set and retry identical roles"),
        );
        assert_eq!(updated.user.permission_version, expected_version);
        assert_eq!(updated.user.role, "admin");
        assert_eq!(role_names(&updated), vec!["admin", "alpha"]);
    }

    let sole_admin_demoted = expect_roles_updated(
        users
            .set_roles(&tenant_a, admin_priority.user.id, vec![roles_a.viewer.id])
            .await
            .expect("demote the only admin"),
    );
    assert_eq!(sole_admin_demoted.user.role, "viewer");

    assert_eq!(
        users
            .set_roles(&tenant_a, owner_priority.user.id, vec![roles_a.viewer.id],)
            .await
            .expect("protect the last owner"),
        SetUserRolesOutcome::WouldRemoveLastOwner
    );
    assert_eq!(
        details(&*users, &tenant_a, owner_priority.user.id)
            .await
            .user
            .permission_version,
        owner_priority.user.permission_version,
        "a rejected last-owner removal does not bump the version"
    );
    assert_eq!(
        users
            .delete(&tenant_a, owner_priority.user.id)
            .await
            .expect("protect last owner delete"),
        DeleteUserOutcome::WouldDeleteLastOwner
    );

    assert_eq!(
        users
            .delete(&tenant_a, upper.user.id)
            .await
            .expect("delete ordinary user"),
        DeleteUserOutcome::Deleted
    );
    assert_eq!(
        users
            .delete(&tenant_a, upper.user.id)
            .await
            .expect("retry ordinary user delete"),
        DeleteUserOutcome::NotFound
    );
    assert_eq!(
        users
            .delete(&tenant_a, i32::MAX)
            .await
            .expect("delete missing user"),
        DeleteUserOutcome::NotFound
    );
}

async fn assert_concurrent_create_and_role_delete(harness: &dyn UserContractHarness) {
    harness
        .reset_users()
        .await
        .expect("reset concurrent-create fixture");
    let users = harness.users();
    let tenant = tenant_a();
    let roles = seed_roles(harness, &tenant).await;

    let left_users = users.clone();
    let right_users = users.clone();
    let left_tenant = tenant.clone();
    let right_tenant = tenant.clone();
    let left = async move {
        left_users
            .create(
                &left_tenant,
                new_user("concurrent-username", "left-hash", None),
            )
            .await
    };
    let right = async move {
        right_users
            .create(
                &right_tenant,
                new_user("concurrent-username", "right-hash", None),
            )
            .await
    };
    let (left, right) = tokio::join!(left, right);
    match (left, right) {
        (Ok(CreateUserOutcome::Created(created)), Err(error))
        | (Err(error), Ok(CreateUserOutcome::Created(created))) => {
            assert_eq!(created.user.permission_version, 2);
            assert_unique_constraint(&error, "users.tenant_username");
        }
        results => panic!("expected one create and one unique violation, got {results:?}"),
    }

    let race_role = harness
        .insert_role(&tenant, "delete-race", false, &["roles.read"])
        .await
        .expect("insert role-delete race fixture");
    let create_users = users.clone();
    let delete_roles = harness.roles();
    let create_tenant = tenant.clone();
    let delete_tenant = tenant.clone();
    let create = async move {
        create_users
            .create(
                &create_tenant,
                new_user(
                    "role-delete-race-user",
                    "race-hash",
                    Some(vec![race_role.id]),
                ),
            )
            .await
    };
    let delete = async move { delete_roles.delete(&delete_tenant, race_role.id).await };
    let (create, delete) = tokio::join!(create, delete);
    match (create, delete) {
        (Ok(CreateUserOutcome::Created(_)), Ok(DeleteRoleOutcome::InUse { user_count: 1, .. }))
        | (Ok(CreateUserOutcome::RolesNotFound), Ok(DeleteRoleOutcome::Deleted)) => {}
        results => panic!(
            "role assignment/deletion race must resolve without a foreign-key leak: {results:?}"
        ),
    }

    // Keep the seed fixture live so the default role path above is exercised.
    assert_eq!(roles.viewer.name, "viewer");
}

async fn assert_concurrent_owner_delete(harness: &dyn UserContractHarness) {
    harness
        .reset_users()
        .await
        .expect("reset concurrent owner-delete fixture");
    let users = harness.users();
    let tenant = tenant_a();
    let roles = seed_roles(harness, &tenant).await;
    let first = create(
        &*users,
        &tenant,
        "owner-delete-1",
        "hash-1",
        Some(vec![roles.owner.id]),
    )
    .await;
    let second = create(
        &*users,
        &tenant,
        "owner-delete-2",
        "hash-2",
        Some(vec![roles.owner.id]),
    )
    .await;

    let left_users = users.clone();
    let right_users = users.clone();
    let left_tenant = tenant.clone();
    let right_tenant = tenant.clone();
    let (left, right) = tokio::join!(
        async move { left_users.delete(&left_tenant, first.user.id).await },
        async move { right_users.delete(&right_tenant, second.user.id).await },
    );
    assert_one_each(
        left.expect("first concurrent owner delete"),
        right.expect("second concurrent owner delete"),
        DeleteUserOutcome::Deleted,
        DeleteUserOutcome::WouldDeleteLastOwner,
    );
    assert_eq!(owner_count(&*users, &tenant).await, 1);
}

async fn assert_concurrent_owner_removal(harness: &dyn UserContractHarness) {
    harness
        .reset_users()
        .await
        .expect("reset concurrent owner-removal fixture");
    let users = harness.users();
    let tenant = tenant_a();
    let roles = seed_roles(harness, &tenant).await;
    let first = create(
        &*users,
        &tenant,
        "owner-remove-1",
        "hash-1",
        Some(vec![roles.owner.id]),
    )
    .await;
    let second = create(
        &*users,
        &tenant,
        "owner-remove-2",
        "hash-2",
        Some(vec![roles.owner.id]),
    )
    .await;

    let left_users = users.clone();
    let right_users = users.clone();
    let left_tenant = tenant.clone();
    let right_tenant = tenant.clone();
    let viewer_id = roles.viewer.id;
    let (left, right) = tokio::join!(
        async move {
            left_users
                .set_roles(&left_tenant, first.user.id, vec![viewer_id])
                .await
        },
        async move {
            right_users
                .set_roles(&right_tenant, second.user.id, vec![viewer_id])
                .await
        },
    );
    let left = left.expect("first concurrent owner removal");
    let right = right.expect("second concurrent owner removal");
    assert!(
        matches!(
            (&left, &right),
            (
                SetUserRolesOutcome::Updated(_),
                SetUserRolesOutcome::WouldRemoveLastOwner
            ) | (
                SetUserRolesOutcome::WouldRemoveLastOwner,
                SetUserRolesOutcome::Updated(_)
            )
        ),
        "exactly one concurrent owner removal may succeed: {left:?}, {right:?}"
    );
    assert_eq!(owner_count(&*users, &tenant).await, 1);
}

async fn assert_concurrent_owner_delete_and_removal(harness: &dyn UserContractHarness) {
    harness
        .reset_users()
        .await
        .expect("reset mixed owner-race fixture");
    let users = harness.users();
    let tenant = tenant_a();
    let roles = seed_roles(harness, &tenant).await;
    let deleted_candidate = create(
        &*users,
        &tenant,
        "owner-mixed-delete",
        "hash-delete",
        Some(vec![roles.owner.id]),
    )
    .await;
    let removed_candidate = create(
        &*users,
        &tenant,
        "owner-mixed-remove",
        "hash-remove",
        Some(vec![roles.owner.id]),
    )
    .await;

    let delete_users = users.clone();
    let remove_users = users.clone();
    let delete_tenant = tenant.clone();
    let remove_tenant = tenant.clone();
    let viewer_id = roles.viewer.id;
    let (deleted, removed) = tokio::join!(
        async move {
            delete_users
                .delete(&delete_tenant, deleted_candidate.user.id)
                .await
        },
        async move {
            remove_users
                .set_roles(&remove_tenant, removed_candidate.user.id, vec![viewer_id])
                .await
        },
    );
    let deleted = deleted.expect("mixed owner delete");
    let removed = removed.expect("mixed owner removal");
    assert!(
        matches!(
            (&deleted, &removed),
            (
                DeleteUserOutcome::Deleted,
                SetUserRolesOutcome::WouldRemoveLastOwner
            ) | (
                DeleteUserOutcome::WouldDeleteLastOwner,
                SetUserRolesOutcome::Updated(_)
            )
        ),
        "delete and demotion must serialize around the owner guard: {deleted:?}, {removed:?}"
    );
    assert_eq!(owner_count(&*users, &tenant).await, 1);
}

async fn seed_roles(harness: &dyn UserContractHarness, tenant: &TenantId) -> RoleFixtures {
    let viewer = harness
        .insert_role(tenant, "viewer", true, &["zones.read"])
        .await
        .expect("insert viewer fixture");
    let owner = harness
        .insert_role(tenant, "owner", true, &["roles.manage"])
        .await
        .expect("insert owner fixture");
    let admin = harness
        .insert_role(tenant, "admin", true, &["roles.read"])
        .await
        .expect("insert admin fixture");
    let binary_upper = harness
        .insert_role(tenant, "Zulu", false, &["zones.read"])
        .await
        .expect("insert uppercase binary fixture");
    let binary_lower = harness
        .insert_role(
            tenant,
            "alpha",
            false,
            &["é.permission", "Alerts.read", "zones.read"],
        )
        .await
        .expect("insert lowercase binary fixture");
    RoleFixtures {
        viewer,
        owner,
        admin,
        binary_upper,
        binary_lower,
    }
}

fn new_user(username: &str, password_hash: &str, role_ids: Option<Vec<i32>>) -> NewUser {
    NewUser {
        username: username.to_owned(),
        password_hash: EncodedPasswordHash::new(password_hash),
        role_ids,
    }
}

async fn create(
    repository: &dyn UserRepository,
    tenant: &TenantId,
    username: &str,
    password_hash: &str,
    role_ids: Option<Vec<i32>>,
) -> UserDetails {
    match repository
        .create(tenant, new_user(username, password_hash, role_ids))
        .await
        .unwrap_or_else(|error| panic!("create user fixture {username}: {error:?}"))
    {
        CreateUserOutcome::Created(details) => details,
        CreateUserOutcome::RolesNotFound => {
            panic!("roles for user fixture {username} are missing")
        }
    }
}

async fn details(repository: &dyn UserRepository, tenant: &TenantId, user_id: i32) -> UserDetails {
    repository
        .get_details(tenant, user_id)
        .await
        .expect("read user details")
        .unwrap_or_else(|| panic!("user fixture {user_id} is missing"))
}

async fn owner_count(repository: &dyn UserRepository, tenant: &TenantId) -> usize {
    repository
        .list(tenant, PageRequest::new(100, 0).unwrap())
        .await
        .expect("list owner-race result")
        .records
        .iter()
        .filter(|details| details.roles.iter().any(|role| role.name == "owner"))
        .count()
}

fn expect_roles_updated(outcome: SetUserRolesOutcome) -> UserDetails {
    match outcome {
        SetUserRolesOutcome::Updated(details) => details,
        other => panic!("expected updated user roles, got {other:?}"),
    }
}

fn usernames(records: &[UserDetails]) -> Vec<&str> {
    records
        .iter()
        .map(|details| details.user.username.as_str())
        .collect()
}

fn role_names(details: &UserDetails) -> Vec<&str> {
    details
        .roles
        .iter()
        .map(|role| role.name.as_str())
        .collect()
}

fn assert_unique_constraint(error: &PersistenceError, expected: &str) {
    assert!(
        matches!(
            error,
            PersistenceError::UniqueViolation { constraint }
                if constraint.as_str() == expected
        ),
        "expected unique constraint {expected}, got {error:?}"
    );
}

fn assert_microsecond(timestamp: DateTime<Utc>) {
    assert_eq!(timestamp.timestamp_subsec_nanos() % 1_000, 0);
}

fn assert_one_each<T>(left: T, right: T, first: T, second: T)
where
    T: Copy + PartialEq + std::fmt::Debug,
{
    assert!(
        (left == first && right == second) || (left == second && right == first),
        "expected one {first:?} and one {second:?}, got {left:?} and {right:?}"
    );
}

fn tenant_a() -> TenantId {
    TenantId::new("users-tenant-a").unwrap()
}

fn tenant_b() -> TenantId {
    TenantId::new("users-tenant-b").unwrap()
}
