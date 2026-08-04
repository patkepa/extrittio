use async_trait::async_trait;
use diesel::Connection;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{NewRole, NewRolePermission, Role};
use crate::db::schema::{role_permissions, roles, user_roles, users};
use crate::domains::identity::role_repository::RoleRepository;
use crate::domains::identity::role_types::{
    CreateRoleRecord, DeleteRoleOutcome, RoleDetails, RoleRecord, UpdateRoleOutcome,
    UpdateRoleRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

fn to_record(row: Role) -> RoleRecord {
    RoleRecord {
        id: row.id,
        name: row.name,
        description: row.description,
        is_system: row.is_system,
        created_at: row.created_at.and_utc(),
        updated_at: row.updated_at.and_utc(),
    }
}

fn hydrate_role(
    connection: &mut PgConnection,
    tenant_id: &str,
    role: Role,
) -> QueryResult<RoleDetails> {
    let permissions = role_permissions::table
        .filter(role_permissions::role_id.eq(role.id))
        .select(role_permissions::permission)
        .order(role_permissions::permission.asc())
        .load::<String>(connection)?;
    let user_count = user_roles::table
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::role_id.eq(role.id))
        .count()
        .get_result(connection)?;
    Ok(RoleDetails {
        role: to_record(role),
        permissions,
        user_count,
    })
}

fn replace_permissions(
    connection: &mut PgConnection,
    role_id: i32,
    permissions: &[String],
) -> QueryResult<()> {
    diesel::delete(role_permissions::table.filter(role_permissions::role_id.eq(role_id)))
        .execute(connection)?;
    if !permissions.is_empty() {
        let rows = permissions
            .iter()
            .map(|permission| NewRolePermission {
                role_id,
                permission: permission.clone(),
            })
            .collect::<Vec<_>>();
        diesel::insert_into(role_permissions::table)
            .values(rows)
            .execute(connection)?;
    }
    Ok(())
}

#[async_trait]
impl RoleRepository for PostgresAdapter {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<RoleDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let rows = roles::table
                    .filter(roles::tenant_id.eq(&tenant_id))
                    .select(Role::as_select())
                    .order((roles::is_system.desc(), roles::name.asc(), roles::id.asc()))
                    .load::<Role>(connection)
                    .map_err(map_diesel_error)?;
                rows.into_iter()
                    .map(|role| hydrate_role(connection, &tenant_id, role))
                    .collect::<QueryResult<Vec<_>>>()
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateRoleRecord,
    ) -> Result<RoleDetails, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let role = diesel::insert_into(roles::table)
                            .values(NewRole {
                                tenant_id: tenant_id.clone(),
                                name: record.name,
                                description: record.description,
                                is_system: false,
                            })
                            .returning(Role::as_returning())
                            .get_result::<Role>(connection)?;
                        replace_permissions(connection, role.id, &record.permissions)?;
                        hydrate_role(connection, &tenant_id, role)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn update(
        &self,
        tenant: &TenantId,
        id: i32,
        record: UpdateRoleRecord,
    ) -> Result<UpdateRoleOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let existing = roles::table
                            .filter(roles::tenant_id.eq(&tenant_id))
                            .filter(roles::id.eq(id))
                            .select(Role::as_select())
                            .for_update()
                            .first::<Role>(connection)
                            .optional()?;
                        let Some(existing) = existing else {
                            return Ok(UpdateRoleOutcome::NotFound);
                        };
                        if existing.is_system {
                            return Ok(UpdateRoleOutcome::SystemRole);
                        }

                        let name = record.name.unwrap_or(existing.name);
                        let description = record.description.unwrap_or(existing.description);
                        let role = diesel::update(
                            roles::table
                                .filter(roles::tenant_id.eq(&tenant_id))
                                .filter(roles::id.eq(id)),
                        )
                        .set((
                            roles::name.eq(name),
                            roles::description.eq(description),
                            roles::updated_at.eq(diesel::dsl::now),
                        ))
                        .returning(Role::as_returning())
                        .get_result::<Role>(connection)?;

                        if let Some(permissions) = record.permissions {
                            replace_permissions(connection, role.id, &permissions)?;
                            let user_ids = user_roles::table
                                .filter(user_roles::tenant_id.eq(&tenant_id))
                                .filter(user_roles::role_id.eq(role.id))
                                .select(user_roles::user_id);
                            diesel::update(
                                users::table
                                    .filter(users::tenant_id.eq(&tenant_id))
                                    .filter(users::id.eq_any(user_ids)),
                            )
                            .set(users::permission_version.eq(users::permission_version + 1))
                            .execute(connection)?;
                        }

                        hydrate_role(connection, &tenant_id, role).map(UpdateRoleOutcome::Updated)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<DeleteRoleOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let role = roles::table
                            .filter(roles::tenant_id.eq(&tenant_id))
                            .filter(roles::id.eq(id))
                            .select(Role::as_select())
                            .for_update()
                            .first::<Role>(connection)
                            .optional()?;
                        let Some(role) = role else {
                            return Ok(DeleteRoleOutcome::NotFound);
                        };
                        if role.is_system {
                            return Ok(DeleteRoleOutcome::SystemRole);
                        }
                        let user_count = user_roles::table
                            .filter(user_roles::tenant_id.eq(&tenant_id))
                            .filter(user_roles::role_id.eq(id))
                            .count()
                            .get_result::<i64>(connection)?;
                        if user_count > 0 {
                            return Ok(DeleteRoleOutcome::InUse {
                                name: role.name,
                                user_count,
                            });
                        }
                        diesel::delete(
                            roles::table
                                .filter(roles::tenant_id.eq(&tenant_id))
                                .filter(roles::id.eq(id)),
                        )
                        .execute(connection)?;
                        Ok(DeleteRoleOutcome::Deleted)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }
}
