use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use extrittio_backend_adapter_tests::{UserContractHarness, users};
use extrittio_backend_core::{PersistenceError, Role, RoleRepository, TenantId, UserRepository};
use extrittio_backend_turso::{TursoDatabase, TursoRoleRepository, TursoUserRepository};
use turso::params;

struct TursoUserHarness {
    _directory: tempfile::TempDir,
    database: TursoDatabase,
    users: Arc<TursoUserRepository>,
    roles: Arc<TursoRoleRepository>,
}

impl TursoUserHarness {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = TursoDatabase::open_and_migrate(
            directory.path(),
            &directory.path().join("user-contract.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let users = Arc::new(TursoUserRepository::new(database.clone()));
        let roles = Arc::new(TursoRoleRepository::new(database.clone()));
        Self {
            _directory: directory,
            database,
            users,
            roles,
        }
    }
}

#[async_trait]
impl UserContractHarness for TursoUserHarness {
    async fn reset_users(&self) -> Result<(), PersistenceError> {
        let handles = self.database.shared_handles();
        let connection = handles.lock_writer().await;
        connection
            .execute_batch(
                "DELETE FROM user_roles
                  WHERE tenant_id IN ('users-tenant-a', 'users-tenant-b');
                 DELETE FROM role_permissions WHERE role_id IN (
                     SELECT id FROM roles
                      WHERE tenant_id IN ('users-tenant-a', 'users-tenant-b')
                 );
                 DELETE FROM roles
                  WHERE tenant_id IN ('users-tenant-a', 'users-tenant-b');
                 DELETE FROM users
                  WHERE tenant_id IN ('users-tenant-a', 'users-tenant-b');",
            )
            .await
            .map_err(internal)?;
        for (id, name) in [
            ("users-tenant-a", "Users Tenant A"),
            ("users-tenant-b", "Users Tenant B"),
        ] {
            connection
                .execute(
                    "INSERT OR IGNORE INTO organizations (id, name, created_at, updated_at)
                     VALUES (?1, ?2, 1, 1)",
                    params![id, name],
                )
                .await
                .map_err(internal)?;
        }
        Ok(())
    }

    fn users(&self) -> Arc<dyn UserRepository> {
        self.users.clone()
    }

    fn roles(&self) -> Arc<dyn RoleRepository> {
        self.roles.clone()
    }

    async fn insert_role(
        &self,
        tenant: &TenantId,
        name: &str,
        is_system: bool,
        permissions: &[&str],
    ) -> Result<Role, PersistenceError> {
        let id = {
            let handles = self.database.shared_handles();
            let mut connection = handles.lock_writer().await;
            let transaction = connection.transaction().await.map_err(internal)?;
            let now = Utc::now().timestamp_micros();
            transaction
                .execute(
                    "INSERT INTO roles (
                        tenant_id, name, is_system, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![tenant.as_str(), name, i64::from(is_system), now],
                )
                .await
                .map_err(internal)?;
            let id = i32::try_from(transaction.last_insert_rowid()).map_err(|_| {
                PersistenceError::CorruptData("roles.id does not fit an i32".into())
            })?;
            for permission in permissions {
                transaction
                    .execute(
                        "INSERT INTO role_permissions (role_id, permission) VALUES (?1, ?2)",
                        params![i64::from(id), *permission],
                    )
                    .await
                    .map_err(internal)?;
            }
            transaction.commit().await.map_err(internal)?;
            id
        };
        self.roles
            .list(tenant)
            .await?
            .into_iter()
            .find(|details| details.role.id == id)
            .map(|details| details.role)
            .ok_or(PersistenceError::NotFound)
    }

    async fn set_user_active(
        &self,
        tenant: &TenantId,
        user_id: i32,
        is_active: bool,
    ) -> Result<(), PersistenceError> {
        let handles = self.database.shared_handles();
        let connection = handles.lock_writer().await;
        let affected = connection
            .execute(
                "UPDATE users SET is_active = ?3 WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(user_id), i64::from(is_active)],
            )
            .await
            .map_err(internal)?;
        if affected == 1 {
            Ok(())
        } else {
            Err(PersistenceError::NotFound)
        }
    }
}

fn internal(error: turso::Error) -> PersistenceError {
    PersistenceError::Internal(error.to_string())
}

#[tokio::test]
async fn turso_satisfies_the_shared_user_contract() {
    users::assert_contract(&TursoUserHarness::new().await).await;
}
