use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::identity::role_repository::RoleRepository;
use crate::domains::identity::role_types::{
    CreateRoleRecord, DeleteRoleOutcome, RoleDetails, UpdateRoleOutcome, UpdateRoleRecord,
};
use crate::error::AppError;
use crate::persistence::PersistenceError;

pub const OWNER_ROLE: &str = "owner";
pub const ADMIN_ROLE: &str = "admin";
pub const OPERATOR_ROLE: &str = "operator";
pub const VIEWER_ROLE: &str = "viewer";

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn RoleRepository,
) -> Result<Vec<RoleDetails>, AppError> {
    policy::require(ctx, Permission::ReadRoles)?;
    Ok(repository.list(ctx.tenant_id()).await?)
}

pub async fn create(
    ctx: &RequestContext,
    repository: &dyn RoleRepository,
    name: &str,
    description: Option<String>,
    permissions: &[String],
) -> Result<RoleDetails, AppError> {
    policy::require(ctx, Permission::ManageRoles)?;

    let name = normalize_role_name(name)?;
    if is_builtin_role_name(&name) {
        return Err(AppError::BadRequest(format!(
            "'{name}' is reserved for a built-in role"
        )));
    }
    let permissions = validate_permission_keys(permissions)?;
    repository
        .create(
            ctx.tenant_id(),
            CreateRoleRecord {
                name,
                description,
                permissions,
            },
        )
        .await
        .map_err(map_role_write_error)
}

pub async fn update(
    ctx: &RequestContext,
    repository: &dyn RoleRepository,
    id: i32,
    name: Option<String>,
    description: Option<Option<String>>,
    permissions: Option<Vec<String>>,
) -> Result<RoleDetails, AppError> {
    policy::require(ctx, Permission::ManageRoles)?;

    let name = name.map(|name| normalize_role_name(&name)).transpose()?;
    if name.as_deref().is_some_and(is_builtin_role_name) {
        return Err(AppError::BadRequest(format!(
            "'{}' is reserved for a built-in role",
            name.as_deref().unwrap_or_default()
        )));
    }
    let permissions = permissions
        .map(|permissions| validate_permission_keys(&permissions))
        .transpose()?;

    match repository
        .update(
            ctx.tenant_id(),
            id,
            UpdateRoleRecord {
                name,
                description,
                permissions,
            },
        )
        .await
        .map_err(map_role_write_error)?
    {
        UpdateRoleOutcome::Updated(role) => Ok(role),
        UpdateRoleOutcome::NotFound => Err(AppError::NotFound(format!("Role {id} not found"))),
        UpdateRoleOutcome::SystemRole => Err(AppError::BadRequest(
            "Built-in roles cannot be modified".into(),
        )),
    }
}

pub async fn delete(
    ctx: &RequestContext,
    repository: &dyn RoleRepository,
    id: i32,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageRoles)?;

    match repository.delete(ctx.tenant_id(), id).await? {
        DeleteRoleOutcome::Deleted => Ok(()),
        DeleteRoleOutcome::NotFound => Err(AppError::NotFound(format!("Role {id} not found"))),
        DeleteRoleOutcome::SystemRole => Err(AppError::BadRequest(
            "Built-in roles cannot be deleted".into(),
        )),
        DeleteRoleOutcome::InUse { name, user_count } => Err(AppError::Conflict(format!(
            "Role '{name}' is assigned to {user_count} user(s)"
        ))),
    }
}

fn map_role_write_error(error: PersistenceError) -> AppError {
    match error {
        PersistenceError::UniqueViolation { .. } => {
            AppError::Conflict("Role name already exists".into())
        }
        other => AppError::Persistence(other),
    }
}

fn normalize_role_name(name: &str) -> Result<String, AppError> {
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Err(AppError::BadRequest("Role name must not be empty".into()));
    }
    if name.len() > 64 {
        return Err(AppError::BadRequest(
            "Role name must be 64 characters or fewer".into(),
        ));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppError::BadRequest(
            "Role name may only contain lowercase letters, numbers, hyphens, and underscores"
                .into(),
        ));
    }
    Ok(name)
}

fn validate_permission_keys(keys: &[String]) -> Result<Vec<String>, AppError> {
    let mut permissions = keys
        .iter()
        .map(|key| key.trim())
        .filter(|key| !key.is_empty())
        .map(|key| {
            Permission::from_key(key)
                .map(|permission| permission.key().to_string())
                .ok_or_else(|| AppError::BadRequest(format!("Unknown permission '{key}'")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    permissions.sort();
    permissions.dedup();
    Ok(permissions)
}

fn is_builtin_role_name(name: &str) -> bool {
    matches!(name, OWNER_ROLE | ADMIN_ROLE | OPERATOR_ROLE | VIEWER_ROLE)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::auth::Claims;
    use crate::domains::identity::role_types::RoleRecord;
    use crate::tenancy::TenantId;

    struct RecordingRepository {
        calls: Mutex<Vec<(String, TenantId)>>,
        delete_outcome: DeleteRoleOutcome,
    }

    impl RecordingRepository {
        fn new(delete_outcome: DeleteRoleOutcome) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                delete_outcome,
            }
        }

        fn details(name: &str, permissions: Vec<String>) -> RoleDetails {
            RoleDetails {
                role: RoleRecord {
                    id: 10,
                    name: name.to_string(),
                    description: None,
                    is_system: false,
                    created_at: Utc.timestamp_opt(1, 0).unwrap(),
                    updated_at: Utc.timestamp_opt(1, 0).unwrap(),
                },
                permissions,
                user_count: 0,
            }
        }

        fn record(&self, operation: &str, tenant: &TenantId) {
            self.calls
                .lock()
                .unwrap()
                .push((operation.to_string(), tenant.clone()));
        }
    }

    #[async_trait]
    impl RoleRepository for RecordingRepository {
        async fn list(&self, tenant: &TenantId) -> Result<Vec<RoleDetails>, PersistenceError> {
            self.record("list", tenant);
            Ok(Vec::new())
        }

        async fn create(
            &self,
            tenant: &TenantId,
            record: CreateRoleRecord,
        ) -> Result<RoleDetails, PersistenceError> {
            self.record("create", tenant);
            Ok(Self::details(&record.name, record.permissions))
        }

        async fn update(
            &self,
            tenant: &TenantId,
            _id: i32,
            record: UpdateRoleRecord,
        ) -> Result<UpdateRoleOutcome, PersistenceError> {
            self.record("update", tenant);
            Ok(UpdateRoleOutcome::Updated(Self::details(
                record.name.as_deref().unwrap_or("custom"),
                record.permissions.unwrap_or_default(),
            )))
        }

        async fn delete(
            &self,
            tenant: &TenantId,
            _id: i32,
        ) -> Result<DeleteRoleOutcome, PersistenceError> {
            self.record("delete", tenant);
            Ok(self.delete_outcome.clone())
        }
    }

    fn context(role: &str, tenant_id: &str) -> RequestContext {
        RequestContext::from_claims(Claims {
            sub: 1,
            username: "operator".to_string(),
            role: role.to_string(),
            tenant_id: Some(tenant_id.to_string()),
            scopes: Vec::new(),
            permission_version: 1,
            exp: 0,
        })
        .expect("test claims contain a valid tenant")
    }

    #[tokio::test]
    async fn normalizes_role_input_and_passes_tenant_identity() {
        let repository = RecordingRepository::new(DeleteRoleOutcome::Deleted);
        let created = create(
            &context("admin", "tenant-a"),
            &repository,
            "  Support_Team ",
            None,
            &["devices.read".to_string(), "devices.read".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(created.role.name, "support_team");
        assert_eq!(created.permissions, vec!["devices.read"]);
        assert_eq!(
            repository.calls.lock().unwrap().as_slice(),
            &[("create".to_string(), TenantId::new("tenant-a").unwrap())]
        );
    }

    #[tokio::test]
    async fn authorization_and_reserved_name_checks_precede_persistence() {
        let repository = RecordingRepository::new(DeleteRoleOutcome::Deleted);

        let error = list(&context("viewer", "tenant-a"), &repository)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Forbidden(_)));

        let error = create(
            &context("admin", "tenant-a"),
            &repository,
            "owner",
            None,
            &[],
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
        assert!(repository.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn maps_atomic_in_use_delete_outcome_to_conflict() {
        let repository = RecordingRepository::new(DeleteRoleOutcome::InUse {
            name: "support".to_string(),
            user_count: 2,
        });

        let error = delete(&context("admin", "tenant-a"), &repository, 10)
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Conflict(message) if message.contains('2')));
    }
}
