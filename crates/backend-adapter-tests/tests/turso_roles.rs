use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use extrittio_backend_adapter_tests::{RoleContractHarness, UserVersionFixture, roles};
use extrittio_backend_core::{PersistenceError, Role, RoleRepository, TenantId};
use extrittio_backend_turso::{TursoDatabase, TursoRoleRepository};
use turso::params;

struct TursoRoleHarness {
    _directory: tempfile::TempDir,
    database: TursoDatabase,
    repository: Arc<TursoRoleRepository>,
}

impl TursoRoleHarness {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = TursoDatabase::open_and_migrate(
            directory.path(),
            &directory.path().join("role-contract.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let repository = Arc::new(TursoRoleRepository::new(database.clone()));
        Self {
            _directory: directory,
            database,
            repository,
        }
    }
}

#[async_trait]
impl RoleContractHarness for TursoRoleHarness {
    async fn reset_roles(&self) -> Result<(), PersistenceError> {
        let handles = self.database.shared_handles();
        let connection = handles.lock_writer().await;
        connection
            .execute_batch(
                "DELETE FROM user_roles WHERE tenant_id IN ('tenant-a', 'tenant-b');
                 DELETE FROM role_permissions WHERE role_id IN (
                     SELECT id FROM roles WHERE tenant_id IN ('tenant-a', 'tenant-b')
                 );
                 DELETE FROM roles WHERE tenant_id IN ('tenant-a', 'tenant-b');
                 DELETE FROM users WHERE tenant_id IN ('tenant-a', 'tenant-b');",
            )
            .await
            .map_err(internal)?;
        for (id, name) in [("tenant-a", "Tenant A"), ("tenant-b", "Tenant B")] {
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

    fn roles(&self) -> Arc<dyn RoleRepository> {
        self.repository.clone()
    }

    async fn insert_system_role(
        &self,
        tenant: &TenantId,
        name: &str,
        description: Option<&str>,
        permissions: &[&str],
    ) -> Result<Role, PersistenceError> {
        let id = {
            let description = description.map(str::to_owned);
            let handles = self.database.shared_handles();
            let mut connection = handles.lock_writer().await;
            let transaction = connection.transaction().await.map_err(internal)?;
            let now = Utc::now().timestamp_micros();
            transaction
                .execute(
                    "INSERT INTO roles (
                        tenant_id, name, description, is_system, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 1, ?4, ?4)",
                    params![tenant.as_str(), name, description, now],
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

        self.repository
            .list(tenant)
            .await?
            .into_iter()
            .find(|details| details.role.id == id)
            .map(|details| details.role)
            .ok_or(PersistenceError::NotFound)
    }

    async fn assign_role_to_user(
        &self,
        tenant: &TenantId,
        role_id: i32,
        username: &str,
    ) -> Result<UserVersionFixture, PersistenceError> {
        let handles = self.database.shared_handles();
        let mut connection = handles.lock_writer().await;
        let transaction = connection.transaction().await.map_err(internal)?;
        transaction
            .execute(
                "INSERT INTO users (
                    tenant_id, username, password_hash, role, created_at
                 ) VALUES (?1, ?2, 'role-contract-password', 'viewer', ?3)",
                params![tenant.as_str(), username, Utc::now().timestamp_micros()],
            )
            .await
            .map_err(internal)?;
        let user_id = i32::try_from(transaction.last_insert_rowid())
            .map_err(|_| PersistenceError::CorruptData("users.id does not fit an i32".into()))?;
        transaction
            .execute(
                "INSERT INTO user_roles (tenant_id, user_id, role_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    tenant.as_str(),
                    i64::from(user_id),
                    i64::from(role_id),
                    Utc::now().timestamp_micros()
                ],
            )
            .await
            .map_err(internal)?;
        let mut rows = transaction
            .query(
                "SELECT permission_version FROM users WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(user_id)],
            )
            .await
            .map_err(internal)?;
        let permission_version = rows
            .next()
            .await
            .map_err(internal)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(internal)?;
        drop(rows);
        transaction.commit().await.map_err(internal)?;
        Ok(UserVersionFixture {
            tenant_id: tenant.clone(),
            user_id,
            permission_version,
        })
    }

    async fn permission_version(&self, user: &UserVersionFixture) -> Result<i32, PersistenceError> {
        let handles = self.database.shared_handles();
        let connection = handles.lock_writer().await;
        let mut rows = connection
            .query(
                "SELECT permission_version FROM users WHERE tenant_id = ?1 AND id = ?2",
                params![user.tenant_id.as_str(), i64::from(user.user_id)],
            )
            .await
            .map_err(internal)?;
        let version = rows
            .next()
            .await
            .map_err(internal)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(internal)?;
        Ok(version)
    }
}

fn internal(error: turso::Error) -> PersistenceError {
    PersistenceError::Internal(error.to_string())
}

#[tokio::test]
async fn turso_satisfies_the_shared_role_contract() {
    roles::assert_contract(&TursoRoleHarness::new().await).await;
}
