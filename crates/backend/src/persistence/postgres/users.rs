use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::Connection;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{NewUser, NewUserRole, Role, User};
use crate::db::schema::{role_permissions, roles, user_roles, users};
use crate::domains::identity::role_types::RoleRecord;
use crate::domains::identity::user_repository::UserRepository;
use crate::domains::identity::user_types::{
    CreateUserOutcome, CreateUserRecord, DeleteUserOutcome, SetUserRolesOutcome, UserCredentials,
    UserDetails, UserList, UserRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

const OWNER_ROLE: &str = "owner";
const ADMIN_ROLE: &str = "admin";
const VIEWER_ROLE: &str = "viewer";

fn to_user_record(row: &User) -> UserRecord {
    UserRecord {
        id: row.id,
        tenant_id: row.tenant_id.clone(),
        username: row.username.clone(),
        role: row.role.clone(),
        created_at: row.created_at.and_utc(),
        is_active: row.is_active,
        permission_version: row.permission_version,
        last_login_at: row.last_login_at.map(|timestamp| timestamp.and_utc()),
    }
}

fn to_role_record(row: Role) -> RoleRecord {
    RoleRecord {
        id: row.id,
        name: row.name,
        description: row.description,
        is_system: row.is_system,
        created_at: row.created_at.and_utc(),
        updated_at: row.updated_at.and_utc(),
    }
}

fn roles_for_user(
    connection: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
) -> QueryResult<Vec<Role>> {
    user_roles::table
        .inner_join(roles::table)
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::user_id.eq(user_id))
        .select(Role::as_select())
        .order((roles::name.asc(), roles::id.asc()))
        .load(connection)
}

fn permissions_for_roles(
    connection: &mut PgConnection,
    role_ids: &[i32],
) -> QueryResult<Vec<String>> {
    if role_ids.is_empty() {
        return Ok(Vec::new());
    }
    role_permissions::table
        .filter(role_permissions::role_id.eq_any(role_ids))
        .select(role_permissions::permission)
        .distinct()
        .order(role_permissions::permission.asc())
        .load(connection)
}

fn hydrate_user(connection: &mut PgConnection, user: User) -> QueryResult<UserDetails> {
    let roles = roles_for_user(connection, &user.tenant_id, user.id)?;
    let role_ids = roles.iter().map(|role| role.id).collect::<Vec<_>>();
    let permissions = permissions_for_roles(connection, &role_ids)?;
    Ok(UserDetails {
        user: to_user_record(&user),
        roles: roles.into_iter().map(to_role_record).collect(),
        permissions,
    })
}

fn load_roles_by_ids(
    connection: &mut PgConnection,
    tenant_id: &str,
    role_ids: &[i32],
) -> QueryResult<Option<Vec<Role>>> {
    let unique_count = role_ids.iter().copied().collect::<HashSet<_>>().len();
    let selected = roles::table
        .filter(roles::tenant_id.eq(tenant_id))
        .filter(roles::id.eq_any(role_ids))
        .select(Role::as_select())
        .order((roles::name.asc(), roles::id.asc()))
        .load::<Role>(connection)?;
    Ok((selected.len() == unique_count).then_some(selected))
}

fn primary_role_name(roles: &[Role]) -> &str {
    roles
        .iter()
        .find(|role| role.name == OWNER_ROLE)
        .or_else(|| roles.iter().find(|role| role.name == ADMIN_ROLE))
        .or_else(|| roles.first())
        .map_or(VIEWER_ROLE, |role| role.name.as_str())
}

fn replace_user_roles(
    connection: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
    selected_roles: &[Role],
) -> QueryResult<User> {
    diesel::delete(
        user_roles::table
            .filter(user_roles::tenant_id.eq(tenant_id))
            .filter(user_roles::user_id.eq(user_id)),
    )
    .execute(connection)?;
    let assignments = selected_roles
        .iter()
        .map(|role| NewUserRole {
            user_id,
            role_id: role.id,
            tenant_id: tenant_id.to_string(),
        })
        .collect::<Vec<_>>();
    diesel::insert_into(user_roles::table)
        .values(assignments)
        .execute(connection)?;
    diesel::update(
        users::table
            .filter(users::tenant_id.eq(tenant_id))
            .filter(users::id.eq(user_id)),
    )
    .set((
        users::role.eq(primary_role_name(selected_roles)),
        users::permission_version.eq(users::permission_version + 1),
    ))
    .returning(User::as_returning())
    .get_result(connection)
}

fn lock_owner_role(connection: &mut PgConnection, tenant_id: &str) -> QueryResult<Option<i32>> {
    roles::table
        .filter(roles::tenant_id.eq(tenant_id))
        .filter(roles::name.eq(OWNER_ROLE))
        .select(roles::id)
        .for_update()
        .first::<i32>(connection)
        .optional()
}

#[async_trait]
impl UserRepository for PostgresAdapter {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<UserList, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let total = users::table
                    .filter(users::tenant_id.eq(&tenant_id))
                    .count()
                    .get_result(connection)
                    .map_err(map_diesel_error)?;
                let rows = users::table
                    .filter(users::tenant_id.eq(&tenant_id))
                    .select(User::as_select())
                    .order((users::username.asc(), users::id.asc()))
                    .limit(limit)
                    .offset(offset)
                    .load::<User>(connection)
                    .map_err(map_diesel_error)?;
                let records = rows
                    .into_iter()
                    .map(|user| hydrate_user(connection, user))
                    .collect::<QueryResult<Vec<_>>>()
                    .map_err(map_diesel_error)?;
                Ok(UserList { records, total })
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateUserRecord,
    ) -> Result<CreateUserOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let selected_roles = match record.role_ids {
                            Some(role_ids) => {
                                let Some(roles) =
                                    load_roles_by_ids(connection, &tenant_id, &role_ids)?
                                else {
                                    return Ok(CreateUserOutcome::RolesNotFound);
                                };
                                roles
                            }
                            None => vec![
                                roles::table
                                    .filter(roles::tenant_id.eq(&tenant_id))
                                    .filter(roles::name.eq(VIEWER_ROLE))
                                    .select(Role::as_select())
                                    .first::<Role>(connection)?,
                            ],
                        };
                        let user = diesel::insert_into(users::table)
                            .values(NewUser {
                                tenant_id: tenant_id.clone(),
                                username: record.username,
                                password_hash: record.password_hash,
                                role: primary_role_name(&selected_roles).to_string(),
                            })
                            .returning(User::as_returning())
                            .get_result::<User>(connection)?;
                        let user =
                            replace_user_roles(connection, &tenant_id, user.id, &selected_roles)?;
                        hydrate_user(connection, user).map(CreateUserOutcome::Created)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn change_password(
        &self,
        tenant: &TenantId,
        user_id: i32,
        password_hash: String,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::update(
                    users::table
                        .filter(users::tenant_id.eq(tenant_id))
                        .filter(users::id.eq(user_id)),
                )
                .set((
                    users::password_hash.eq(password_hash),
                    users::permission_version.eq(users::permission_version + 1),
                ))
                .execute(connection)
                .map(|rows| rows > 0)
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<DeleteUserOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let owner_role_id = lock_owner_role(connection, &tenant_id)?;
                        let user = users::table
                            .filter(users::tenant_id.eq(&tenant_id))
                            .filter(users::id.eq(user_id))
                            .select(User::as_select())
                            .for_update()
                            .first::<User>(connection)
                            .optional()?;
                        if user.is_none() {
                            return Ok(DeleteUserOutcome::NotFound);
                        }
                        if let Some(owner_role_id) = owner_role_id {
                            let is_owner = user_roles::table
                                .filter(user_roles::tenant_id.eq(&tenant_id))
                                .filter(user_roles::user_id.eq(user_id))
                                .filter(user_roles::role_id.eq(owner_role_id))
                                .count()
                                .get_result::<i64>(connection)?
                                > 0;
                            if is_owner {
                                let owner_count = user_roles::table
                                    .filter(user_roles::tenant_id.eq(&tenant_id))
                                    .filter(user_roles::role_id.eq(owner_role_id))
                                    .select(user_roles::user_id)
                                    .distinct()
                                    .count()
                                    .get_result::<i64>(connection)?;
                                if owner_count <= 1 {
                                    return Ok(DeleteUserOutcome::WouldDeleteLastOwner);
                                }
                            }
                        }
                        diesel::delete(
                            users::table
                                .filter(users::tenant_id.eq(&tenant_id))
                                .filter(users::id.eq(user_id)),
                        )
                        .execute(connection)?;
                        Ok(DeleteUserOutcome::Deleted)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn set_roles(
        &self,
        tenant: &TenantId,
        user_id: i32,
        role_ids: Vec<i32>,
    ) -> Result<SetUserRolesOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let owner_role_id = lock_owner_role(connection, &tenant_id)?;
                        let user = users::table
                            .filter(users::tenant_id.eq(&tenant_id))
                            .filter(users::id.eq(user_id))
                            .select(User::as_select())
                            .for_update()
                            .first::<User>(connection)
                            .optional()?;
                        if user.is_none() {
                            return Ok(SetUserRolesOutcome::UserNotFound);
                        }
                        let Some(selected_roles) =
                            load_roles_by_ids(connection, &tenant_id, &role_ids)?
                        else {
                            return Ok(SetUserRolesOutcome::RolesNotFound);
                        };

                        if let Some(owner_role_id) = owner_role_id {
                            let currently_owner = user_roles::table
                                .filter(user_roles::tenant_id.eq(&tenant_id))
                                .filter(user_roles::user_id.eq(user_id))
                                .filter(user_roles::role_id.eq(owner_role_id))
                                .count()
                                .get_result::<i64>(connection)?
                                > 0;
                            let remains_owner =
                                selected_roles.iter().any(|role| role.id == owner_role_id);
                            if currently_owner && !remains_owner {
                                let owner_count = user_roles::table
                                    .filter(user_roles::tenant_id.eq(&tenant_id))
                                    .filter(user_roles::role_id.eq(owner_role_id))
                                    .select(user_roles::user_id)
                                    .distinct()
                                    .count()
                                    .get_result::<i64>(connection)?;
                                if owner_count <= 1 {
                                    return Ok(SetUserRolesOutcome::WouldRemoveLastOwner);
                                }
                            }
                        }

                        let user =
                            replace_user_roles(connection, &tenant_id, user_id, &selected_roles)?;
                        hydrate_user(connection, user).map(SetUserRolesOutcome::Updated)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn find_credentials_by_username(
        &self,
        tenant: &TenantId,
        username: &str,
    ) -> Result<Option<UserCredentials>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let username = username.to_owned();
        self.executor
            .run(move |connection| {
                let user = users::table
                    .filter(users::tenant_id.eq(tenant_id))
                    .filter(users::username.eq(username))
                    .select(User::as_select())
                    .first::<User>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                user.map(|user| {
                    let password_hash = user.password_hash.clone();
                    hydrate_user(connection, user).map(|details| UserCredentials {
                        details,
                        password_hash,
                    })
                })
                .transpose()
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_details(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<Option<UserDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let user = users::table
                    .filter(users::tenant_id.eq(tenant_id))
                    .filter(users::id.eq(user_id))
                    .select(User::as_select())
                    .first::<User>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                user.map(|user| hydrate_user(connection, user))
                    .transpose()
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn record_successful_login(
        &self,
        tenant: &TenantId,
        user_id: i32,
        logged_in_at: DateTime<Utc>,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::update(
                    users::table
                        .filter(users::tenant_id.eq(tenant_id))
                        .filter(users::id.eq(user_id)),
                )
                .set(users::last_login_at.eq(logged_in_at.naive_utc()))
                .execute(connection)
                .map(|rows| rows > 0)
                .map_err(map_diesel_error)
            })
            .await
    }
}
