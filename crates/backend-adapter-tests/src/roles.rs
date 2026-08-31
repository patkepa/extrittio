use std::sync::Arc;

use async_trait::async_trait;
use extrittio_backend_core::{
    DeleteRoleOutcome, NewRole, PersistenceError, Role, RoleDetails, RolePatch, RoleRepository,
    TenantId, UpdateRoleOutcome,
};

/// A user assigned to a role by a contract harness.
///
/// The original version is retained so the shared suite can prove exact,
/// atomic permission-version increments without depending on adapter models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserVersionFixture {
    pub tenant_id: TenantId,
    pub user_id: i32,
    pub permission_version: i32,
}

/// Independent lifecycle and fixture boundary for the shared role contracts.
///
/// This intentionally does not extend the zone [`crate::ContractHarness`].
#[async_trait]
pub trait RoleContractHarness: Send + Sync {
    /// Restore the `tenant-a` and `tenant-b` role fixtures to an empty state.
    async fn reset_roles(&self) -> Result<(), PersistenceError>;

    fn roles(&self) -> Arc<dyn RoleRepository>;

    /// Insert a system role, which cannot be created through the public port.
    async fn insert_system_role(
        &self,
        tenant: &TenantId,
        name: &str,
        description: Option<&str>,
        permissions: &[&str],
    ) -> Result<Role, PersistenceError>;

    /// Create a user and atomically assign the requested role to that user.
    async fn assign_role_to_user(
        &self,
        tenant: &TenantId,
        role_id: i32,
        username: &str,
    ) -> Result<UserVersionFixture, PersistenceError>;

    /// Read the current credential invalidation version for a fixture user.
    async fn permission_version(&self, user: &UserVersionFixture) -> Result<i32, PersistenceError>;
}

/// Execute the canonical role persistence semantics against one adapter.
pub async fn assert_contract(harness: &dyn RoleContractHarness) {
    harness
        .reset_roles()
        .await
        .expect("reset role contract fixture");
    let repository = harness.roles();
    let tenant_a = TenantId::new("tenant-a").unwrap();
    let tenant_b = TenantId::new("tenant-b").unwrap();

    let system_unicode = harness
        .insert_system_role(
            &tenant_a,
            "é-system",
            Some("Unicode system role"),
            &[
                "é.permission",
                "zones.read",
                "Alerts.read",
                "api_keys.manage",
            ],
        )
        .await
        .expect("insert Unicode system role");
    let system_ascii = harness
        .insert_system_role(
            &tenant_a,
            "System",
            Some("ASCII system role"),
            &["roles.read"],
        )
        .await
        .expect("insert ASCII system role");

    let custom_zero = create(&*repository, &tenant_a, "0-custom", None, &[]).await;
    let custom_zulu = create(
        &*repository,
        &tenant_a,
        "zulu",
        Some("duplicate target"),
        &["roles.read"],
    )
    .await;
    let custom_alpha = create(
        &*repository,
        &tenant_a,
        "alpha",
        Some("original description"),
        &["zones.read", "roles.read"],
    )
    .await;
    let custom_eclair = create(
        &*repository,
        &tenant_a,
        "eclair",
        None,
        &["zones.read", "alerts.read", "api_keys.manage"],
    )
    .await;
    let same_name_other_tenant = create(
        &*repository,
        &tenant_b,
        "zulu",
        Some("same name, other tenant"),
        &[],
    )
    .await;

    assert_eq!(custom_zulu.role.tenant_id, tenant_a);
    assert_eq!(same_name_other_tenant.role.tenant_id, tenant_b);
    assert_eq!(same_name_other_tenant.role.name, custom_zulu.role.name);
    assert_microsecond_precision(&custom_zero);
    assert_microsecond_precision(&custom_eclair);

    let listed = repository.list(&tenant_a).await.expect("list tenant roles");
    assert_eq!(
        listed
            .iter()
            .map(|details| details.role.name.as_str())
            .collect::<Vec<_>>(),
        vec!["System", "é-system", "0-custom", "alpha", "eclair", "zulu"],
        "roles are ordered by system DESC, binary name ASC, then id ASC"
    );
    assert_eq!(
        details_by_id(&listed, system_unicode.id)
            .permissions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            "Alerts.read",
            "api_keys.manage",
            "zones.read",
            "é.permission",
        ],
        "permissions use binary ascending order"
    );
    assert_eq!(
        repository
            .list(&tenant_a)
            .await
            .expect("repeat stable role list"),
        listed,
        "unchanged role lists are stable"
    );
    assert_eq!(
        repository
            .list(&tenant_b)
            .await
            .expect("list other tenant roles")
            .iter()
            .map(|details| details.role.id)
            .collect::<Vec<_>>(),
        vec![same_name_other_tenant.role.id],
        "role lists are tenant isolated"
    );

    let duplicate = repository
        .create(
            &tenant_a,
            NewRole {
                name: custom_zulu.role.name.clone(),
                description: None,
                permissions: vec![],
            },
        )
        .await
        .expect_err("role names are unique within a tenant");
    assert_unique_constraint(&duplicate, "roles.tenant_name");

    assert_eq!(
        repository
            .update(
                &tenant_b,
                custom_alpha.role.id,
                RolePatch {
                    name: Some("cross-tenant-update".into()),
                    description: None,
                    permissions: None,
                },
            )
            .await
            .expect("cross-tenant update"),
        UpdateRoleOutcome::NotFound
    );
    assert_eq!(
        repository
            .delete(&tenant_b, custom_alpha.role.id)
            .await
            .expect("cross-tenant delete"),
        DeleteRoleOutcome::NotFound
    );
    assert_eq!(
        repository
            .delete(&tenant_a, i32::MAX)
            .await
            .expect("delete missing role"),
        DeleteRoleOutcome::NotFound
    );
    assert_eq!(
        repository
            .update(
                &tenant_a,
                i32::MAX,
                RolePatch {
                    name: None,
                    description: None,
                    permissions: None,
                },
            )
            .await
            .expect("update missing role"),
        UpdateRoleOutcome::NotFound
    );

    assert_eq!(
        repository
            .update(
                &tenant_a,
                system_ascii.id,
                RolePatch {
                    name: Some("system-mutated".into()),
                    description: Some(None),
                    permissions: Some(vec![]),
                },
            )
            .await
            .expect("reject system-role update"),
        UpdateRoleOutcome::SystemRole
    );
    assert_eq!(
        repository
            .delete(&tenant_a, system_unicode.id)
            .await
            .expect("reject system-role delete"),
        DeleteRoleOutcome::SystemRole
    );

    let assigned = harness
        .assign_role_to_user(&tenant_a, custom_alpha.role.id, "role-contract-user-1")
        .await
        .expect("assign rollback target role");
    assert_eq!(
        harness
            .permission_version(&assigned)
            .await
            .expect("read initial permission version"),
        assigned.permission_version
    );

    let before_rejected = find(&*repository, &tenant_a, custom_alpha.role.id).await;
    let rejected = repository
        .update(
            &tenant_a,
            custom_alpha.role.id,
            RolePatch {
                name: Some(custom_zulu.role.name.clone()),
                description: Some(Some("must roll back".into())),
                permissions: Some(vec!["alerts.read".into()]),
            },
        )
        .await
        .expect_err("duplicate multi-field update must fail atomically");
    assert_unique_constraint(&rejected, "roles.tenant_name");
    let after_rejected = find(&*repository, &tenant_a, custom_alpha.role.id).await;
    assert_details_equal(&after_rejected, &before_rejected);
    assert_eq!(
        harness
            .permission_version(&assigned)
            .await
            .expect("read version after rejected update"),
        assigned.permission_version,
        "a rolled-back permission replacement cannot invalidate credentials"
    );

    let metadata_updated = expect_updated(
        repository
            .update(
                &tenant_a,
                custom_alpha.role.id,
                RolePatch {
                    name: None,
                    description: Some(Some("metadata changed".into())),
                    permissions: None,
                },
            )
            .await
            .expect("metadata-only update"),
    );
    assert!(metadata_updated.role.updated_at > before_rejected.role.updated_at);
    assert_microsecond_precision(&metadata_updated);
    assert_eq!(
        harness
            .permission_version(&assigned)
            .await
            .expect("read version after metadata update"),
        assigned.permission_version,
        "metadata-only updates do not invalidate credentials"
    );

    let permissions_updated = expect_updated(
        repository
            .update(
                &tenant_a,
                custom_alpha.role.id,
                RolePatch {
                    name: None,
                    description: None,
                    permissions: Some(vec!["zones.read".into(), "alerts.read".into()]),
                },
            )
            .await
            .expect("permission-only update"),
    );
    assert!(permissions_updated.role.updated_at > metadata_updated.role.updated_at);
    assert_eq!(
        permissions_updated
            .permissions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["alerts.read", "zones.read"]
    );
    assert_microsecond_precision(&permissions_updated);
    assert_eq!(
        harness
            .permission_version(&assigned)
            .await
            .expect("read version after permission update"),
        assigned.permission_version + 1,
        "supplying permissions increments each assignee exactly once"
    );

    let empty_updated = expect_updated(
        repository
            .update(
                &tenant_a,
                custom_alpha.role.id,
                RolePatch {
                    name: None,
                    description: None,
                    permissions: None,
                },
            )
            .await
            .expect("empty role patch"),
    );
    assert!(empty_updated.role.updated_at > permissions_updated.role.updated_at);
    assert_microsecond_precision(&empty_updated);
    assert_eq!(
        harness
            .permission_version(&assigned)
            .await
            .expect("read version after empty update"),
        assigned.permission_version + 1,
        "an empty patch advances the role timestamp without invalidating credentials"
    );
    let repeated_empty_updated = expect_updated(
        repository
            .update(
                &tenant_a,
                custom_alpha.role.id,
                RolePatch {
                    name: None,
                    description: None,
                    permissions: None,
                },
            )
            .await
            .expect("immediately repeated empty role patch"),
    );
    assert!(repeated_empty_updated.role.updated_at > empty_updated.role.updated_at);
    assert_microsecond_precision(&repeated_empty_updated);
    assert_eq!(
        harness
            .permission_version(&assigned)
            .await
            .expect("read version after repeated empty update"),
        assigned.permission_version + 1
    );

    let second_assignee = harness
        .assign_role_to_user(&tenant_a, custom_alpha.role.id, "role-contract-user-2")
        .await
        .expect("assign role to a second user");
    assert_eq!(
        repository
            .delete(&tenant_a, custom_alpha.role.id)
            .await
            .expect("delete assigned role"),
        DeleteRoleOutcome::InUse {
            name: custom_alpha.role.name.clone(),
            user_count: 2,
        }
    );
    let still_present = find(&*repository, &tenant_a, custom_alpha.role.id).await;
    assert_eq!(still_present.user_count, 2);
    assert_eq!(
        harness
            .permission_version(&second_assignee)
            .await
            .expect("read second assignee version"),
        second_assignee.permission_version
    );

    assert_eq!(
        repository
            .delete(&tenant_a, custom_zero.role.id)
            .await
            .expect("delete unassigned role"),
        DeleteRoleOutcome::Deleted
    );
    assert!(
        repository
            .list(&tenant_a)
            .await
            .expect("list after delete")
            .iter()
            .all(|details| details.role.id != custom_zero.role.id)
    );
    assert_eq!(
        repository
            .delete(&tenant_a, custom_zero.role.id)
            .await
            .expect("retry unassigned role delete"),
        DeleteRoleOutcome::NotFound
    );

    assert_concurrent_duplicate_create(repository.clone(), &tenant_a).await;
    assert_concurrent_permission_updates(harness, repository, &tenant_a).await;
}

async fn assert_concurrent_duplicate_create(
    repository: Arc<dyn RoleRepository>,
    tenant: &TenantId,
) {
    let left_repository = repository.clone();
    let right_repository = repository;
    let left_tenant = tenant.clone();
    let right_tenant = tenant.clone();
    let left = async move {
        left_repository
            .create(
                &left_tenant,
                NewRole {
                    name: "concurrent-duplicate".into(),
                    description: Some("left contender".into()),
                    permissions: vec![],
                },
            )
            .await
    };
    let right = async move {
        right_repository
            .create(
                &right_tenant,
                NewRole {
                    name: "concurrent-duplicate".into(),
                    description: Some("right contender".into()),
                    permissions: vec![],
                },
            )
            .await
    };

    let (left, right) = tokio::join!(left, right);
    match (left, right) {
        (Ok(_), Err(error)) | (Err(error), Ok(_)) => {
            assert_unique_constraint(&error, "roles.tenant_name");
        }
        results => panic!("expected one create and one unique violation, got {results:?}"),
    }
}

async fn assert_concurrent_permission_updates(
    harness: &dyn RoleContractHarness,
    repository: Arc<dyn RoleRepository>,
    tenant: &TenantId,
) {
    let role = create(
        &*repository,
        tenant,
        "concurrent-permissions",
        None,
        &["roles.read"],
    )
    .await;
    let user = harness
        .assign_role_to_user(tenant, role.role.id, "role-contract-concurrent-user")
        .await
        .expect("assign concurrent-update role");
    let initial_version = harness
        .permission_version(&user)
        .await
        .expect("read concurrent-update baseline version");

    let left_repository = repository.clone();
    let right_repository = repository.clone();
    let left_tenant = tenant.clone();
    let right_tenant = tenant.clone();
    let role_id = role.role.id;
    let left = async move {
        left_repository
            .update(
                &left_tenant,
                role_id,
                RolePatch {
                    name: None,
                    description: None,
                    permissions: Some(vec!["alerts.read".into()]),
                },
            )
            .await
    };
    let right = async move {
        right_repository
            .update(
                &right_tenant,
                role_id,
                RolePatch {
                    name: None,
                    description: None,
                    permissions: Some(vec!["zones.read".into()]),
                },
            )
            .await
    };

    let (left, right) = tokio::join!(left, right);
    assert!(matches!(left, Ok(UpdateRoleOutcome::Updated(_))));
    assert!(matches!(right, Ok(UpdateRoleOutcome::Updated(_))));
    assert_eq!(
        harness
            .permission_version(&user)
            .await
            .expect("read version after concurrent permission updates"),
        initial_version + 2,
        "concurrent replacements cannot lose permission-version increments"
    );

    let final_role = find(&*repository, tenant, role_id).await;
    let final_permissions = final_role
        .permissions
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert!(
        final_permissions == vec!["alerts.read"] || final_permissions == vec!["zones.read"],
        "each update is a complete replacement; the last committer wins"
    );
}

async fn create(
    repository: &dyn RoleRepository,
    tenant: &TenantId,
    name: &str,
    description: Option<&str>,
    permissions: &[&str],
) -> RoleDetails {
    repository
        .create(
            tenant,
            NewRole {
                name: name.into(),
                description: description.map(str::to_owned),
                permissions: permissions
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("create role fixture {name}: {error:?}"))
}

async fn find(repository: &dyn RoleRepository, tenant: &TenantId, id: i32) -> RoleDetails {
    repository
        .list(tenant)
        .await
        .expect("list roles to find fixture")
        .into_iter()
        .find(|details| details.role.id == id)
        .unwrap_or_else(|| panic!("role fixture {id} is missing"))
}

fn details_by_id(roles: &[RoleDetails], id: i32) -> &RoleDetails {
    roles
        .iter()
        .find(|details| details.role.id == id)
        .unwrap_or_else(|| panic!("role fixture {id} is missing"))
}

fn expect_updated(outcome: UpdateRoleOutcome) -> RoleDetails {
    match outcome {
        UpdateRoleOutcome::Updated(details) => details,
        other => panic!("expected updated role, got {other:?}"),
    }
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

fn assert_microsecond_precision(details: &RoleDetails) {
    assert_eq!(details.role.created_at.timestamp_subsec_nanos() % 1_000, 0);
    assert_eq!(details.role.updated_at.timestamp_subsec_nanos() % 1_000, 0);
}

fn assert_details_equal(actual: &RoleDetails, expected: &RoleDetails) {
    assert_eq!(actual.role.id, expected.role.id);
    assert_eq!(actual.role.tenant_id, expected.role.tenant_id);
    assert_eq!(actual.role.name, expected.role.name);
    assert_eq!(actual.role.description, expected.role.description);
    assert_eq!(actual.role.is_system, expected.role.is_system);
    assert_eq!(actual.role.created_at, expected.role.created_at);
    assert_eq!(actual.role.updated_at, expected.role.updated_at);
    assert_eq!(actual.permissions, expected.permissions);
    assert_eq!(actual.user_count, expected.user_count);
}
