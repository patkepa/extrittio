use async_trait::async_trait;
use diesel::Connection;
use diesel::prelude::*;
use diesel_migrations::MigrationHarness;

use crate::db::models::{
    NewDeviceType, NewServerConfigEntry, NewUser, NewUserRole, Role, ServerConfigEntry, User,
};
use crate::db::schema::{device_types, roles, server_config, user_roles, users};
use crate::persistence::bootstrap::{
    BootstrapOwner, BootstrapRepository, BuiltinDeviceType, DatabaseHealth, SeedOwnerOutcome,
};
use crate::persistence::error::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

#[derive(diesel::QueryableByName)]
struct HealthRow {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    value: i32,
}

#[async_trait]
impl BootstrapRepository for PostgresAdapter {
    async fn health(&self) -> Result<DatabaseHealth, PersistenceError> {
        self.executor
            .run(|connection| {
                let row = diesel::sql_query("SELECT 1 AS value")
                    .get_result::<HealthRow>(connection)
                    .map_err(map_diesel_error)?;
                if row.value != 1 {
                    return Err(PersistenceError::CorruptData(
                        "database health query returned an unexpected value".to_string(),
                    ));
                }
                Ok(DatabaseHealth { reachable: true })
            })
            .await
    }

    async fn run_migrations(&self) -> Result<(), PersistenceError> {
        self.executor
            .run(|connection| {
                connection
                    .run_pending_migrations(crate::MIGRATIONS)
                    .map(|_| ())
                    .map_err(|error| PersistenceError::Migration(error.to_string()))
            })
            .await
    }

    async fn seed_builtin_device_types(
        &self,
        tenant: &TenantId,
        records: Vec<BuiltinDeviceType>,
    ) -> Result<(), PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let rows = records
                    .into_iter()
                    .map(|record| NewDeviceType {
                        tenant_id: tenant_id.clone(),
                        name: record.name,
                        icon: record.icon,
                        color_hex: record.color_hex,
                    })
                    .collect::<Vec<_>>();
                diesel::insert_into(device_types::table)
                    .values(rows)
                    .on_conflict((device_types::tenant_id, device_types::name))
                    .do_nothing()
                    .execute(connection)
                    .map(|_| ())
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_or_create_server_config(
        &self,
        key: &str,
        generated_value: String,
    ) -> Result<String, PersistenceError> {
        let key = key.to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        diesel::insert_into(server_config::table)
                            .values(NewServerConfigEntry {
                                key: key.clone(),
                                value: generated_value,
                            })
                            .on_conflict(server_config::key)
                            .do_nothing()
                            .execute(connection)?;
                        server_config::table
                            .find(key)
                            .select(ServerConfigEntry::as_select())
                            .first::<ServerConfigEntry>(connection)
                            .map(|entry| entry.value)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn seed_owner_if_empty(
        &self,
        tenant: &TenantId,
        owner: BootstrapOwner,
    ) -> Result<SeedOwnerOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        diesel::sql_query("LOCK TABLE users IN SHARE ROW EXCLUSIVE MODE")
                            .execute(connection)?;
                        if users::table.count().get_result::<i64>(connection)? > 0 {
                            return Ok(SeedOwnerOutcome::SkippedUsersExist);
                        }
                        let owner_role = roles::table
                            .filter(roles::tenant_id.eq(&tenant_id))
                            .filter(roles::name.eq("owner"))
                            .select(Role::as_select())
                            .first::<Role>(connection)?;
                        let user = diesel::insert_into(users::table)
                            .values(NewUser {
                                tenant_id: tenant_id.clone(),
                                username: owner.username,
                                password_hash: owner.password_hash,
                                role: "owner".to_string(),
                            })
                            .returning(User::as_returning())
                            .get_result::<User>(connection)?;
                        diesel::insert_into(user_roles::table)
                            .values(NewUserRole {
                                user_id: user.id,
                                role_id: owner_role.id,
                                tenant_id: tenant_id.clone(),
                            })
                            .execute(connection)?;
                        diesel::update(
                            users::table
                                .filter(users::tenant_id.eq(&tenant_id))
                                .filter(users::id.eq(user.id)),
                        )
                        .set(users::permission_version.eq(users::permission_version + 1))
                        .execute(connection)?;
                        Ok(SeedOwnerOutcome::Created)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn users_exist(&self) -> Result<bool, PersistenceError> {
        self.executor
            .run(move |connection| {
                users::table
                    .count()
                    .get_result::<i64>(connection)
                    .map(|count| count > 0)
                    .map_err(map_diesel_error)
            })
            .await
    }
}
