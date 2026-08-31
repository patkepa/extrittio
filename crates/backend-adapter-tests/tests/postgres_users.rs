use std::sync::Arc;

use async_trait::async_trait;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::{Bool, Integer, Text};
use diesel::{Connection, QueryableByName, RunQueryDsl};
use extrittio_backend_adapter_tests::{UserContractHarness, users};
use extrittio_backend_core::{PersistenceError, Role, RoleRepository, TenantId, UserRepository};
use extrittio_backend_postgres::{
    PostgresRoleRepository, PostgresUserRepository, run_pending_migrations,
};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = Integer)]
    id: i32,
}

struct PostgresUserHarness {
    pool: Pool<ConnectionManager<diesel::PgConnection>>,
    users: Arc<PostgresUserRepository>,
    roles: Arc<PostgresRoleRepository>,
}

impl PostgresUserHarness {
    fn from_environment() -> Option<Self> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = Pool::builder()
            .max_size(6)
            .build(ConnectionManager::new(url))
            .expect("connect to disposable PostgreSQL user-contract database");
        run_pending_migrations(&mut pool.get().unwrap()).expect("run PostgreSQL migrations");
        let users = Arc::new(PostgresUserRepository::from_pool(pool.clone()));
        let roles = Arc::new(PostgresRoleRepository::from_pool(pool.clone()));
        Some(Self { pool, users, roles })
    }
}

#[async_trait]
impl UserContractHarness for PostgresUserHarness {
    async fn reset_users(&self) -> Result<(), PersistenceError> {
        let mut connection = self
            .pool
            .get()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        connection
            .transaction::<_, diesel::result::Error, _>(|connection| {
                for statement in [
                    "DELETE FROM user_roles
                     WHERE tenant_id IN ('users-tenant-a', 'users-tenant-b')",
                    "DELETE FROM role_permissions WHERE role_id IN (
                         SELECT id FROM roles
                          WHERE tenant_id IN ('users-tenant-a', 'users-tenant-b')
                     )",
                    "DELETE FROM roles
                     WHERE tenant_id IN ('users-tenant-a', 'users-tenant-b')",
                    "DELETE FROM users
                     WHERE tenant_id IN ('users-tenant-a', 'users-tenant-b')",
                    "INSERT INTO organizations (id, name)
                     VALUES ('users-tenant-a', 'Users Tenant A'),
                            ('users-tenant-b', 'Users Tenant B')
                     ON CONFLICT (id) DO NOTHING",
                ] {
                    diesel::sql_query(statement).execute(connection)?;
                }
                Ok(())
            })
            .map_err(|error| PersistenceError::Internal(error.to_string()))
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
            let mut connection = self
                .pool
                .get()
                .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
            connection
                .transaction::<_, diesel::result::Error, _>(|connection| {
                    let role = diesel::sql_query(
                        "INSERT INTO roles (tenant_id, name, is_system)
                         VALUES ($1, $2, $3) RETURNING id",
                    )
                    .bind::<Text, _>(tenant.as_str())
                    .bind::<Text, _>(name)
                    .bind::<Bool, _>(is_system)
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
        let mut connection = self
            .pool
            .get()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let affected =
            diesel::sql_query("UPDATE users SET is_active = $3 WHERE tenant_id = $1 AND id = $2")
                .bind::<Text, _>(tenant.as_str())
                .bind::<Integer, _>(user_id)
                .bind::<Bool, _>(is_active)
                .execute(&mut connection)
                .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        if affected == 1 {
            Ok(())
        } else {
            Err(PersistenceError::NotFound)
        }
    }
}

#[tokio::test]
async fn postgres_satisfies_the_shared_user_contract_when_configured() {
    let Some(harness) = PostgresUserHarness::from_environment() else {
        eprintln!("skipping PostgreSQL user contract: DATABASE_URL is not set");
        return;
    };
    users::assert_contract(&harness).await;
}
