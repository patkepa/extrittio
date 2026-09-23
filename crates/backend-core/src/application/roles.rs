use std::sync::Arc;

use crate::application::require_permission;
use crate::{ADMIN_ROLE, OPERATOR_ROLE, OWNER_ROLE, VIEWER_ROLE};
use crate::{
    ApplicationError, DeleteRoleOutcome, NewRole, Permission, RoleDetails, RolePatch,
    RoleRepository, TenantContext, UpdateRoleOutcome,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRole {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleUpdate {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct RoleApplication {
    repository: Arc<dyn RoleRepository>,
}

impl RoleApplication {
    #[must_use]
    pub fn new(repository: Arc<dyn RoleRepository>) -> Self {
        Self { repository }
    }

    pub async fn list(
        &self,
        context: &TenantContext,
    ) -> Result<Vec<RoleDetails>, ApplicationError> {
        require_permission(context, Permission::ReadRoles)?;
        Ok(self.repository.list(context.tenant_id()).await?)
    }

    pub fn available_permissions(
        &self,
        context: &TenantContext,
    ) -> Result<&'static [Permission], ApplicationError> {
        require_permission(context, Permission::ReadRoles)?;
        Ok(Permission::all())
    }

    pub async fn create(
        &self,
        context: &TenantContext,
        input: CreateRole,
    ) -> Result<RoleDetails, ApplicationError> {
        require_permission(context, Permission::ManageRoles)?;
        let name = normalize_role_name(&input.name)?;
        if is_builtin_role_name(&name) {
            return Err(ApplicationError::InvalidInput(format!(
                "'{name}' is reserved for a built-in role"
            )));
        }
        let permissions = validate_permission_keys(&input.permissions)?;
        self.repository
            .create(
                context.tenant_id(),
                NewRole {
                    name,
                    description: input.description,
                    permissions,
                },
            )
            .await
            .map_err(map_role_write_error)
    }

    pub async fn update(
        &self,
        context: &TenantContext,
        role_id: i32,
        update: RoleUpdate,
    ) -> Result<RoleDetails, ApplicationError> {
        require_permission(context, Permission::ManageRoles)?;
        let name = update
            .name
            .map(|name| normalize_role_name(&name))
            .transpose()?;
        if name.as_deref().is_some_and(is_builtin_role_name) {
            return Err(ApplicationError::InvalidInput(format!(
                "'{}' is reserved for a built-in role",
                name.as_deref().unwrap_or_default()
            )));
        }
        let permissions = update
            .permissions
            .map(|permissions| validate_permission_keys(&permissions))
            .transpose()?;

        match self
            .repository
            .update(
                context.tenant_id(),
                role_id,
                RolePatch {
                    name,
                    description: update.description,
                    permissions,
                },
            )
            .await
            .map_err(map_role_write_error)?
        {
            UpdateRoleOutcome::Updated(role) => Ok(role),
            UpdateRoleOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "Role {role_id} not found"
            ))),
            UpdateRoleOutcome::SystemRole => Err(ApplicationError::InvalidInput(
                "Built-in roles cannot be modified".into(),
            )),
        }
    }

    pub async fn delete(
        &self,
        context: &TenantContext,
        role_id: i32,
    ) -> Result<(), ApplicationError> {
        require_permission(context, Permission::ManageRoles)?;
        match self.repository.delete(context.tenant_id(), role_id).await? {
            DeleteRoleOutcome::Deleted => Ok(()),
            DeleteRoleOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "Role {role_id} not found"
            ))),
            DeleteRoleOutcome::SystemRole => Err(ApplicationError::InvalidInput(
                "Built-in roles cannot be deleted".into(),
            )),
            DeleteRoleOutcome::InUse { name, user_count } => Err(ApplicationError::Conflict(
                format!("Role '{name}' is assigned to {user_count} user(s)"),
            )),
        }
    }
}

fn map_role_write_error(error: crate::PersistenceError) -> ApplicationError {
    match error {
        crate::PersistenceError::UniqueViolation { .. } => {
            ApplicationError::Conflict("Role name already exists".into())
        }
        other => ApplicationError::Persistence(other),
    }
}

fn normalize_role_name(name: &str) -> Result<String, ApplicationError> {
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Err(ApplicationError::InvalidInput(
            "Role name must not be empty".into(),
        ));
    }
    if name.len() > 64 {
        return Err(ApplicationError::InvalidInput(
            "Role name must be 64 characters or fewer".into(),
        ));
    }
    if !name.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '_' | '-')
    }) {
        return Err(ApplicationError::InvalidInput(
            "Role name may only contain lowercase letters, numbers, hyphens, and underscores"
                .into(),
        ));
    }
    Ok(name)
}

fn validate_permission_keys(keys: &[String]) -> Result<Vec<String>, ApplicationError> {
    let mut permissions = keys
        .iter()
        .map(|key| key.trim())
        .filter(|key| !key.is_empty())
        .map(|key| {
            Permission::from_key(key)
                .map(|permission| permission.key().to_string())
                .ok_or_else(|| {
                    ApplicationError::InvalidInput(format!("Unknown permission '{key}'"))
                })
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
    use futures::executor::block_on;

    use super::*;
    use crate::{Actor, ConstraintName, PermissionSet, PersistenceError, Role, TenantId};

    struct RecordingRepository {
        calls: Mutex<Vec<(String, TenantId)>>,
        update_outcome: Mutex<Option<UpdateRoleOutcome>>,
        delete_outcome: DeleteRoleOutcome,
        create_error: Mutex<Option<PersistenceError>>,
    }

    impl RecordingRepository {
        fn new(delete_outcome: DeleteRoleOutcome) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                update_outcome: Mutex::new(None),
                delete_outcome,
                create_error: Mutex::new(None),
            }
        }

        fn details(tenant: &TenantId, name: &str, permissions: Vec<String>) -> RoleDetails {
            RoleDetails {
                role: Role {
                    id: 10,
                    tenant_id: tenant.clone(),
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
            role: NewRole,
        ) -> Result<RoleDetails, PersistenceError> {
            self.record("create", tenant);
            if let Some(error) = self.create_error.lock().unwrap().take() {
                return Err(error);
            }
            Ok(Self::details(tenant, &role.name, role.permissions))
        }

        async fn update(
            &self,
            tenant: &TenantId,
            _role_id: i32,
            patch: RolePatch,
        ) -> Result<UpdateRoleOutcome, PersistenceError> {
            self.record("update", tenant);
            Ok(self
                .update_outcome
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| {
                    UpdateRoleOutcome::Updated(Self::details(
                        tenant,
                        patch.name.as_deref().unwrap_or("custom"),
                        patch.permissions.unwrap_or_default(),
                    ))
                }))
        }

        async fn delete(
            &self,
            tenant: &TenantId,
            _role_id: i32,
        ) -> Result<DeleteRoleOutcome, PersistenceError> {
            self.record("delete", tenant);
            Ok(self.delete_outcome.clone())
        }
    }

    fn context(tenant: &str, permissions: &[&str]) -> TenantContext {
        TenantContext::new(
            TenantId::new(tenant).unwrap(),
            Actor::User {
                id: 1,
                username: "operator".into(),
                role: "viewer".into(),
            },
            PermissionSet::from_keys(permissions),
        )
    }

    #[test]
    fn normalizes_input_and_passes_the_tenant() {
        block_on(async {
            let repository = Arc::new(RecordingRepository::new(DeleteRoleOutcome::Deleted));
            let application = RoleApplication::new(repository.clone());
            let created = application
                .create(
                    &context("tenant-a", &["roles.manage"]),
                    CreateRole {
                        name: "  Support_Team ".into(),
                        description: None,
                        permissions: vec!["devices.read".into(), "devices.read".into(), "".into()],
                    },
                )
                .await
                .unwrap();

            assert_eq!(created.role.name, "support_team");
            assert_eq!(created.permissions, vec!["devices.read"]);
            assert_eq!(created.role.tenant_id.as_str(), "tenant-a");
            assert_eq!(repository.calls.lock().unwrap().len(), 1);
        });
    }

    #[test]
    fn authorization_and_validation_precede_persistence() {
        block_on(async {
            let repository = Arc::new(RecordingRepository::new(DeleteRoleOutcome::Deleted));
            let application = RoleApplication::new(repository.clone());

            let forbidden = application
                .list(&context("tenant-a", &["devices.read"]))
                .await
                .unwrap_err();
            assert!(
                matches!(forbidden, ApplicationError::Forbidden(message) if message == "Missing permission 'roles.read'")
            );

            for (name, message) in [
                ("owner", "'owner' is reserved for a built-in role"),
                ("   ", "Role name must not be empty"),
                (
                    "not valid",
                    "Role name may only contain lowercase letters, numbers, hyphens, and underscores",
                ),
            ] {
                let error = application
                    .create(
                        &context("tenant-a", &["roles.manage"]),
                        CreateRole {
                            name: name.into(),
                            description: None,
                            permissions: Vec::new(),
                        },
                    )
                    .await
                    .unwrap_err();
                assert!(
                    matches!(error, ApplicationError::InvalidInput(actual) if actual == message)
                );
            }
            assert!(repository.calls.lock().unwrap().is_empty());
        });
    }

    #[test]
    fn preserves_role_outcome_and_conflict_messages() {
        block_on(async {
            let repository = Arc::new(RecordingRepository::new(DeleteRoleOutcome::InUse {
                name: "support".into(),
                user_count: 2,
            }));
            let application = RoleApplication::new(repository.clone());
            *repository.update_outcome.lock().unwrap() = Some(UpdateRoleOutcome::SystemRole);
            let update = application
                .update(
                    &context("tenant-a", &["roles.manage"]),
                    10,
                    RoleUpdate {
                        name: None,
                        description: None,
                        permissions: None,
                    },
                )
                .await
                .unwrap_err();
            assert!(
                matches!(update, ApplicationError::InvalidInput(message) if message == "Built-in roles cannot be modified")
            );

            let delete = application
                .delete(&context("tenant-a", &["roles.manage"]), 10)
                .await
                .unwrap_err();
            assert!(
                matches!(delete, ApplicationError::Conflict(message) if message == "Role 'support' is assigned to 2 user(s)")
            );

            *repository.create_error.lock().unwrap() = Some(PersistenceError::UniqueViolation {
                constraint: ConstraintName::new("roles.tenant_name"),
            });
            let duplicate = application
                .create(
                    &context("tenant-a", &["roles.manage"]),
                    CreateRole {
                        name: "support".into(),
                        description: None,
                        permissions: Vec::new(),
                    },
                )
                .await
                .unwrap_err();
            assert!(
                matches!(duplicate, ApplicationError::Conflict(message) if message == "Role name already exists")
            );
        });
    }

    #[test]
    fn permission_catalog_preserves_the_public_order() {
        let repository = Arc::new(RecordingRepository::new(DeleteRoleOutcome::Deleted));
        let application = RoleApplication::new(repository);
        let keys = application
            .available_permissions(&context("tenant-a", &["roles.read"]))
            .unwrap()
            .iter()
            .map(|permission| permission.key())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "firmware.deploy",
                "alerts.manage",
                "device_blueprints.manage",
                "devices.manage",
                "api_keys.manage",
                "firmware.manage",
                "fleets.manage",
                "rules.manage",
                "roles.manage",
                "shadows.manage",
                "users.manage",
                "zones.manage",
                "commands.read",
                "alerts.read",
                "device_blueprints.read",
                "devices.read",
                "fleets.read",
                "firmware.read",
                "logs.read",
                "rules.read",
                "roles.read",
                "server_metrics.read",
                "shadows.read",
                "telemetry.read",
                "users.read",
                "zones.read",
                "commands.send",
            ]
        );
    }
}
