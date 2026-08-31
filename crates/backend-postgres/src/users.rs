//! PostgreSQL implementation of tenant-scoped users and role assignments.

use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::Connection;
use diesel::PgConnection;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::Text;
use extrittio_backend_core::{
    ADMIN_ROLE, ChangePasswordOutcome, CreateUserOutcome, DeleteUserOutcome, EncodedPasswordHash,
    NewUser, OWNER_ROLE, PageRequest, PersistenceError, RecordSuccessfulLoginOutcome, Role,
    SetUserRolesOutcome, TenantId, User, UserAuthEpoch, UserCredentials, UserDetails, UserPage,
    UserRepository, VIEWER_ROLE,
};
use uuid::Uuid;

use crate::error::map_diesel_error;
use crate::models::{NewUser as NewUserRow, NewUserRole, Role as RoleRow};
use crate::schema::{role_permissions, roles, user_roles, users};
use crate::{PostgresExecutor, PostgresPool};

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct StoredUser {
    id: i32,
    tenant_id: String,
    username: String,
    role: String,
    created_at: NaiveDateTime,
    is_active: bool,
    permission_version: i32,
    last_login_at: Option<NaiveDateTime>,
    auth_epoch: String,
}

fn into_user(row: &StoredUser) -> QueryResult<User> {
    Ok(User {
        id: row.id,
        tenant_id: TenantId::new(row.tenant_id.clone())
            .map_err(|error| diesel::result::Error::DeserializationError(Box::new(error)))?,
        username: row.username.clone(),
        role: row.role.clone(),
        created_at: row.created_at.and_utc(),
        is_active: row.is_active,
        permission_version: row.permission_version,
        auth_epoch: UserAuthEpoch::new(row.auth_epoch.clone()),
        last_login_at: row.last_login_at.map(|timestamp| timestamp.and_utc()),
    })
}

fn into_role(row: RoleRow) -> QueryResult<Role> {
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

fn roles_for_user(
    connection: &mut PgConnection,
    tenant_id: &str,
    user_id: i32,
) -> QueryResult<Vec<RoleRow>> {
    user_roles::table
        .inner_join(roles::table)
        .filter(user_roles::tenant_id.eq(tenant_id))
        .filter(user_roles::user_id.eq(user_id))
        .select(RoleRow::as_select())
        .order((
            sql::<Text>(r#"roles.name COLLATE "C""#).asc(),
            roles::id.asc(),
        ))
        .load(connection)
}

fn permissions_for_roles(
    connection: &mut PgConnection,
    role_ids: &[i32],
) -> QueryResult<Vec<String>> {
    if role_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut permissions = role_permissions::table
        .filter(role_permissions::role_id.eq_any(role_ids))
        .select(role_permissions::permission)
        .order(sql::<Text>(r#"permission COLLATE "C""#).asc())
        .load(connection)?;
    permissions.dedup();
    Ok(permissions)
}

fn hydrate_user(connection: &mut PgConnection, row: StoredUser) -> QueryResult<UserDetails> {
    let assigned = roles_for_user(connection, &row.tenant_id, row.id)?;
    let role_ids = assigned.iter().map(|role| role.id).collect::<Vec<_>>();
    let permissions = permissions_for_roles(connection, &role_ids)?;
    Ok(UserDetails {
        user: into_user(&row)?,
        roles: assigned
            .into_iter()
            .map(into_role)
            .collect::<QueryResult<_>>()?,
        permissions,
    })
}

/// Load and share-lock selected roles until the caller's transaction commits.
///
/// The lock makes an assignment race with role deletion resolve as either a
/// complete assignment or `RolesNotFound`, never as a leaked foreign-key error.
fn lock_roles_by_ids(
    connection: &mut PgConnection,
    tenant_id: &str,
    role_ids: &[i32],
) -> QueryResult<Option<Vec<RoleRow>>> {
    let unique_count = role_ids.iter().copied().collect::<HashSet<_>>().len();
    if unique_count == 0 {
        return Ok(Some(Vec::new()));
    }
    let selected = roles::table
        .filter(roles::tenant_id.eq(tenant_id))
        .filter(roles::id.eq_any(role_ids))
        .select(RoleRow::as_select())
        .order((sql::<Text>(r#"name COLLATE "C""#).asc(), roles::id.asc()))
        .for_share()
        .load::<RoleRow>(connection)?;
    Ok((selected.len() == unique_count).then_some(selected))
}

fn lock_default_viewer(
    connection: &mut PgConnection,
    tenant_id: &str,
) -> QueryResult<Option<Vec<RoleRow>>> {
    roles::table
        .filter(roles::tenant_id.eq(tenant_id))
        .filter(roles::name.eq(VIEWER_ROLE))
        .select(RoleRow::as_select())
        .for_share()
        .first::<RoleRow>(connection)
        .optional()
        .map(|role| role.map(|role| vec![role]))
}

fn primary_role_name(roles: &[RoleRow]) -> &str {
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
    selected_roles: &[RoleRow],
) -> QueryResult<StoredUser> {
    diesel::delete(
        user_roles::table
            .filter(user_roles::tenant_id.eq(tenant_id))
            .filter(user_roles::user_id.eq(user_id)),
    )
    .execute(connection)?;
    if !selected_roles.is_empty() {
        let assignments = selected_roles
            .iter()
            .map(|role| NewUserRole {
                user_id,
                role_id: role.id,
                tenant_id: tenant_id.to_owned(),
            })
            .collect::<Vec<_>>();
        diesel::insert_into(user_roles::table)
            .values(assignments)
            .execute(connection)?;
    }
    diesel::update(
        users::table
            .filter(users::tenant_id.eq(tenant_id))
            .filter(users::id.eq(user_id)),
    )
    .set((
        users::role.eq(primary_role_name(selected_roles)),
        users::permission_version.eq(users::permission_version + 1),
    ))
    .returning(StoredUser::as_returning())
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

/// PostgreSQL user adapter. Clones share the application's bounded pool.
#[derive(Clone)]
pub struct PostgresUserRepository {
    executor: PostgresExecutor,
}

impl PostgresUserRepository {
    #[must_use]
    pub fn new(executor: PostgresExecutor) -> Self {
        Self { executor }
    }

    #[must_use]
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self::new(PostgresExecutor::new(pool))
    }

    #[must_use]
    pub fn executor(&self) -> &PostgresExecutor {
        &self.executor
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        page: PageRequest,
    ) -> Result<UserPage, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let limit = page.limit();
        let offset = page.offset();
        self.executor
            .run(move |connection| {
                let total = users::table
                    .filter(users::tenant_id.eq(&tenant_id))
                    .count()
                    .get_result(connection)
                    .map_err(map_diesel_error)?;
                let rows = users::table
                    .filter(users::tenant_id.eq(&tenant_id))
                    .select(StoredUser::as_select())
                    .order((
                        sql::<Text>(r#"username COLLATE "C""#).asc(),
                        users::id.asc(),
                    ))
                    .limit(limit)
                    .offset(offset)
                    .load::<StoredUser>(connection)
                    .map_err(map_diesel_error)?;
                let records = rows
                    .into_iter()
                    .map(|user| hydrate_user(connection, user))
                    .collect::<QueryResult<Vec<_>>>()
                    .map_err(map_diesel_error)?;
                Ok(UserPage { records, total })
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        user: NewUser,
    ) -> Result<CreateUserOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let selected_roles = match user.role_ids {
                            Some(role_ids) => {
                                let Some(roles) =
                                    lock_roles_by_ids(connection, &tenant_id, &role_ids)?
                                else {
                                    return Ok(CreateUserOutcome::RolesNotFound);
                                };
                                roles
                            }
                            None => {
                                let Some(viewer) = lock_default_viewer(connection, &tenant_id)?
                                else {
                                    return Ok(CreateUserOutcome::RolesNotFound);
                                };
                                viewer
                            }
                        };
                        let row = diesel::insert_into(users::table)
                            .values(NewUserRow {
                                tenant_id: tenant_id.clone(),
                                username: user.username,
                                password_hash: user.password_hash.into_inner(),
                                role: primary_role_name(&selected_roles).to_owned(),
                                auth_epoch: Uuid::new_v4().to_string(),
                            })
                            .returning(StoredUser::as_returning())
                            .get_result::<StoredUser>(connection)?;
                        let row =
                            replace_user_roles(connection, &tenant_id, row.id, &selected_roles)?;
                        hydrate_user(connection, row).map(CreateUserOutcome::Created)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn change_password(
        &self,
        tenant: &TenantId,
        user_id: i32,
        password_hash: EncodedPasswordHash,
    ) -> Result<ChangePasswordOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let changed = diesel::update(
                    users::table
                        .filter(users::tenant_id.eq(tenant_id))
                        .filter(users::id.eq(user_id)),
                )
                .set((
                    users::password_hash.eq(password_hash.into_inner()),
                    users::permission_version.eq(users::permission_version + 1),
                ))
                .execute(connection)
                .map_err(map_diesel_error)?;
                Ok(if changed == 0 {
                    ChangePasswordOutcome::NotFound
                } else {
                    ChangePasswordOutcome::PasswordChanged
                })
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
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let owner_role_id = lock_owner_role(connection, &tenant_id)?;
                        let user = users::table
                            .filter(users::tenant_id.eq(&tenant_id))
                            .filter(users::id.eq(user_id))
                            .select(StoredUser::as_select())
                            .for_update()
                            .first::<StoredUser>(connection)
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
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let owner_role_id = lock_owner_role(connection, &tenant_id)?;
                        let user_exists = users::table
                            .filter(users::tenant_id.eq(&tenant_id))
                            .filter(users::id.eq(user_id))
                            .select(users::id)
                            .first::<i32>(connection)
                            .optional()?;
                        if user_exists.is_none() {
                            return Ok(SetUserRolesOutcome::UserNotFound);
                        }
                        let Some(selected_roles) =
                            lock_roles_by_ids(connection, &tenant_id, &role_ids)?
                        else {
                            return Ok(SetUserRolesOutcome::RolesNotFound);
                        };
                        let user = users::table
                            .filter(users::tenant_id.eq(&tenant_id))
                            .filter(users::id.eq(user_id))
                            .select(StoredUser::as_select())
                            .for_update()
                            .first::<StoredUser>(connection)
                            .optional()?;
                        if user.is_none() {
                            return Ok(SetUserRolesOutcome::UserNotFound);
                        }

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

                        let row =
                            replace_user_roles(connection, &tenant_id, user_id, &selected_roles)?;
                        hydrate_user(connection, row).map(SetUserRolesOutcome::Updated)
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
                connection
                    .build_transaction()
                    .repeatable_read()
                    .read_only()
                    .run(|connection| {
                        let row = users::table
                            .filter(users::tenant_id.eq(tenant_id))
                            .filter(users::username.eq(username))
                            .select((StoredUser::as_select(), users::password_hash))
                            .first::<(StoredUser, String)>(connection)
                            .optional()?;
                        row.map(|(row, password_hash)| {
                            let password_hash = EncodedPasswordHash::new(password_hash);
                            hydrate_user(connection, row).map(|details| UserCredentials {
                                details,
                                password_hash,
                            })
                        })
                        .transpose()
                    })
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
                connection
                    .build_transaction()
                    .repeatable_read()
                    .read_only()
                    .run(|connection| {
                        users::table
                            .filter(users::tenant_id.eq(tenant_id))
                            .filter(users::id.eq(user_id))
                            .select(StoredUser::as_select())
                            .first::<StoredUser>(connection)
                            .optional()?
                            .map(|row| hydrate_user(connection, row))
                            .transpose()
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn record_successful_login(
        &self,
        tenant: &TenantId,
        user_id: i32,
        logged_in_at: DateTime<Utc>,
    ) -> Result<RecordSuccessfulLoginOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let changed = diesel::update(
                    users::table
                        .filter(users::tenant_id.eq(tenant_id))
                        .filter(users::id.eq(user_id)),
                )
                .set(users::last_login_at.eq(logged_in_at.naive_utc()))
                .execute(connection)
                .map_err(map_diesel_error)?;
                Ok(if changed == 0 {
                    RecordSuccessfulLoginOutcome::NotFound
                } else {
                    RecordSuccessfulLoginOutcome::LoginRecorded
                })
            })
            .await
    }
}
