use std::sync::Arc;

use async_trait::async_trait;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::{Integer, Text};
use diesel::{Connection, OptionalExtension, QueryableByName, RunQueryDsl};
use extrittio_backend_adapter_tests::{RoleContractHarness, UserVersionFixture, roles};
use extrittio_backend_core::{PersistenceError, Role, RoleRepository, TenantId};
use extrittio_backend_postgres::{PostgresRoleRepository, run_pending_migrations};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = Integer)]
    id: i32,
}

#[derive(QueryableByName)]
struct UserVersionRow {
    #[diesel(sql_type = Integer)]
    id: i32,
    #[diesel(sql_type = Integer)]
    permission_version: i32,
}

#[derive(QueryableByName)]
struct VersionRow {
    #[diesel(sql_type = Integer)]
    permission_version: i32,
}

struct PostgresRoleHarness {
    pool: Pool<ConnectionManager<diesel::PgConnection>>,
    repository: Arc<PostgresRoleRepository>,
}

impl PostgresRoleHarness {
    fn from_environment() -> Option<Self> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = Pool::builder()
            .max_size(4)
            .build(ConnectionManager::new(url))
            .expect("connect to disposable PostgreSQL role-contract database");
        run_pending_migrations(&mut pool.get().unwrap()).expect("run PostgreSQL migrations");
        let repository = Arc::new(PostgresRoleRepository::from_pool(pool.clone()));
        Some(Self { pool, repository })
    }
}

#[async_trait]
impl RoleContractHarness for PostgresRoleHarness {
    async fn reset_roles(&self) -> Result<(), PersistenceError> {
        let mut connection = self
            .pool
            .get()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        connection
            .transaction::<_, diesel::result::Error, _>(|connection| {
                for statement in [
                    "DELETE FROM user_roles WHERE tenant_id IN ('tenant-a', 'tenant-b')",
                    "DELETE FROM role_permissions WHERE role_id IN (
                         SELECT id FROM roles WHERE tenant_id IN ('tenant-a', 'tenant-b')
                     )",
                    "DELETE FROM roles WHERE tenant_id IN ('tenant-a', 'tenant-b')",
                    "DELETE FROM users WHERE tenant_id IN ('tenant-a', 'tenant-b')",
                    "INSERT INTO organizations (id, name)
                     VALUES ('tenant-a', 'Tenant A'), ('tenant-b', 'Tenant B')
                     ON CONFLICT (id) DO NOTHING",
                ] {
                    diesel::sql_query(statement).execute(connection)?;
                }
                Ok(())
            })
            .map_err(|error| PersistenceError::Internal(error.to_string()))
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
            let mut connection = self
                .pool
                .get()
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
            connection
                .transaction::<_, diesel::result::Error, _>(|connection| {
                    let role = diesel::sql_query(
                        "INSERT INTO roles (tenant_id, name, description, is_system)
                         VALUES ($1, $2, $3, TRUE)
                         RETURNING id",
                    )
                    .bind::<Text, _>(tenant.as_str())
                    .bind::<Text, _>(name)
                    .bind::<diesel::sql_types::Nullable<Text>, _>(description)
                    .get_result::<IdRow>(connection)?;
                    for permission in permissions {
                        diesel::sql_query(
                            "INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2)",
                        )
                        .bind::<Integer, _>(role.id)
                        .bind::<Text, _>(*permission)
                        .execute(connection)?;
                    }
                    Ok(role.id)
                })
                .map_err(|error| PersistenceError::Internal(error.to_string()))?
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
        let mut connection = self
            .pool
            .get()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let row = connection
            .transaction::<_, diesel::result::Error, _>(|connection| {
                let user = diesel::sql_query(
                    "INSERT INTO users (tenant_id, username, password_hash, role)
                     VALUES ($1, $2, 'role-contract-password', 'viewer')
                     RETURNING id, permission_version",
                )
                .bind::<Text, _>(tenant.as_str())
                .bind::<Text, _>(username)
                .get_result::<UserVersionRow>(connection)?;
                diesel::sql_query(
                    "INSERT INTO user_roles (tenant_id, user_id, role_id)
                     VALUES ($1, $2, $3)",
                )
                .bind::<Text, _>(tenant.as_str())
                .bind::<Integer, _>(user.id)
                .bind::<Integer, _>(role_id)
                .execute(connection)?;
                Ok(user)
            })
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        Ok(UserVersionFixture {
            tenant_id: tenant.clone(),
            user_id: row.id,
            permission_version: row.permission_version,
        })
    }

    async fn permission_version(&self, user: &UserVersionFixture) -> Result<i32, PersistenceError> {
        let mut connection = self
            .pool
            .get()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        diesel::sql_query("SELECT permission_version FROM users WHERE tenant_id = $1 AND id = $2")
            .bind::<Text, _>(user.tenant_id.as_str())
            .bind::<Integer, _>(user.user_id)
            .get_result::<VersionRow>(&mut connection)
            .optional()
            .map_err(|error| PersistenceError::Internal(error.to_string()))?
            .map(|row| row.permission_version)
            .ok_or(PersistenceError::NotFound)
    }
}

#[tokio::test]
async fn postgres_satisfies_the_shared_role_contract_when_configured() {
    let Some(harness) = PostgresRoleHarness::from_environment() else {
        eprintln!("skipping PostgreSQL role contract: DATABASE_URL is not set");
        return;
    };
    roles::assert_contract(&harness).await;
}
