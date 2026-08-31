//! PostgreSQL implementation of tenant-scoped roles and permissions.

use async_trait::async_trait;
use diesel::Connection;
use diesel::PgConnection;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::Text;
use extrittio_backend_core::{
    DeleteRoleOutcome, NewRole, PersistenceError, Role, RoleDetails, RolePatch, RoleRepository,
    TenantId, UpdateRoleOutcome,
};

use crate::error::map_diesel_error;
use crate::models::{NewRole as NewRoleRow, NewRolePermission, Role as RoleRow};
use crate::schema::{role_permissions, roles, user_roles, users};
use crate::{PostgresExecutor, PostgresPool};

fn into_domain(row: RoleRow) -> QueryResult<Role> {
    Ok(Role {
        id: row.id,
        tenant_id: TenantId::new(row.tenant_id)
            .map_err(|error| diesel::result::Error::DeserializationError(Box::new(error)))?,
        name: row.name,
        description: row.description,
        is_system: row.is_system,
        created_at: row.created_at.and_utc(),
        updated_at: row.updated_at.and_utc(),
    })
}

fn hydrate_role(
    connection: &mut PgConnection,
    tenant_id: &str,
    role: RoleRow,
) -> QueryResult<RoleDetails> {
    let permissions = role_permissions::table
        .filter(role_permissions::role_id.eq(role.id))
        .select(role_permissions::permission)
        .order(sql::<Text>(r#"permission COLLATE "C""#).asc())
        .load::<String>(connection)?;
    let user_count = user_roles::table
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::role_id.eq(role.id))
        .count()
        .get_result(connection)?;
    Ok(RoleDetails {
        role: into_domain(role)?,
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

#[derive(Clone)]
pub struct PostgresRoleRepository {
    executor: PostgresExecutor,
}

impl PostgresRoleRepository {
    #[must_use]
    pub fn new(executor: PostgresExecutor) -> Self {
        Self { executor }
    }

    #[must_use]
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self::new(PostgresExecutor::new(pool))
    }
}

#[async_trait]
impl RoleRepository for PostgresRoleRepository {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<RoleDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let rows = roles::table
                    .filter(roles::tenant_id.eq(&tenant_id))
                    .select(RoleRow::as_select())
                    .order((
                        roles::is_system.desc(),
                        sql::<Text>(r#"name COLLATE "C""#).asc(),
                        roles::id.asc(),
                    ))
                    .load::<RoleRow>(connection)
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
        role: NewRole,
    ) -> Result<RoleDetails, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let row = diesel::insert_into(roles::table)
                            .values(NewRoleRow {
                                tenant_id: tenant_id.clone(),
                                name: role.name,
                                description: role.description,
                                is_system: false,
                            })
                            .returning(RoleRow::as_returning())
                            .get_result::<RoleRow>(connection)?;
                        replace_permissions(connection, row.id, &role.permissions)?;
                        hydrate_role(connection, &tenant_id, row)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn update(
        &self,
        tenant: &TenantId,
        role_id: i32,
        patch: RolePatch,
    ) -> Result<UpdateRoleOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let existing = roles::table
                            .filter(roles::tenant_id.eq(&tenant_id))
                            .filter(roles::id.eq(role_id))
                            .select(RoleRow::as_select())
                            .for_update()
                            .first::<RoleRow>(connection)
                            .optional()?;
                        let Some(existing) = existing else {
                            return Ok(UpdateRoleOutcome::NotFound);
                        };
                        if existing.is_system {
                            return Ok(UpdateRoleOutcome::SystemRole);
                        }

                        let name = patch.name.unwrap_or(existing.name);
                        let description = patch.description.unwrap_or(existing.description);
                        let role = diesel::update(
                            roles::table
                                .filter(roles::tenant_id.eq(&tenant_id))
                                .filter(roles::id.eq(role_id)),
                        )
                        .set((
                            roles::name.eq(name),
                            roles::description.eq(description),
                            roles::updated_at.eq(diesel::dsl::now),
                        ))
                        .returning(RoleRow::as_returning())
                        .get_result::<RoleRow>(connection)?;

                        if let Some(permissions) = patch.permissions {
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
        role_id: i32,
    ) -> Result<DeleteRoleOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let role = roles::table
                            .filter(roles::tenant_id.eq(&tenant_id))
                            .filter(roles::id.eq(role_id))
                            .select(RoleRow::as_select())
                            .for_update()
                            .first::<RoleRow>(connection)
                            .optional()?;
                        let Some(role) = role else {
                            return Ok(DeleteRoleOutcome::NotFound);
                        };
                        if role.is_system {
                            return Ok(DeleteRoleOutcome::SystemRole);
                        }
                        let user_count = user_roles::table
                            .filter(user_roles::tenant_id.eq(&tenant_id))
                            .filter(user_roles::role_id.eq(role_id))
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
                                .filter(roles::id.eq(role_id)),
                        )
                        .execute(connection)?;
                        Ok(DeleteRoleOutcome::Deleted)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }
}
