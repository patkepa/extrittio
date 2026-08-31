//! Turso implementation of tenant-scoped users and role assignments.

use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use extrittio_backend_core::{
    ADMIN_ROLE, ChangePasswordOutcome, CreateUserOutcome, DeleteUserOutcome, EncodedPasswordHash,
    NewUser, OWNER_ROLE, PageRequest, PersistenceError, RecordSuccessfulLoginOutcome, Role,
    SetUserRolesOutcome, TenantId, User, UserCredentials, UserDetails, UserPage, UserRepository,
    VIEWER_ROLE,
};
use turso::{Connection, Row, params};

use crate::error::map_error;
use crate::row;
use crate::{TursoConnectionHandles, TursoDatabase};

const USER_COLUMNS: &str = "id, tenant_id, username, role, created_at, is_active, \
                           permission_version, last_login_at";
const ROLE_COLUMNS: &str = "id, tenant_id, name, description, is_system, created_at, updated_at";

fn decode_user(record: &Row) -> Result<User, PersistenceError> {
    let tenant_id: String = record.get(1).map_err(map_error)?;
    Ok(User {
        id: row::i32(record.get::<i64>(0).map_err(map_error)?, "users.id")?,
        tenant_id: TenantId::new(tenant_id)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        username: record.get(2).map_err(map_error)?,
        role: record.get(3).map_err(map_error)?,
        created_at: row::datetime(record.get(4).map_err(map_error)?)?,
        is_active: record.get::<i64>(5).map_err(map_error)? != 0,
        permission_version: row::i32(
            record.get(6).map_err(map_error)?,
            "users.permission_version",
        )?,
        last_login_at: record
            .get::<Option<i64>>(7)
            .map_err(map_error)?
            .map(row::datetime)
            .transpose()?,
    })
}

fn decode_role(record: &Row) -> Result<Role, PersistenceError> {
    let tenant_id: String = record.get(1).map_err(map_error)?;
    Ok(Role {
        id: row::i32(record.get::<i64>(0).map_err(map_error)?, "roles.id")?,
        tenant_id: TenantId::new(tenant_id)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        name: record.get(2).map_err(map_error)?,
        description: record.get(3).map_err(map_error)?,
        is_system: record.get::<i64>(4).map_err(map_error)? != 0,
        created_at: row::datetime(record.get(5).map_err(map_error)?)?,
        updated_at: row::datetime(record.get(6).map_err(map_error)?)?,
    })
}

async fn hydrate(
    connection: &Connection,
    tenant: &TenantId,
    user_id: i32,
) -> Result<Option<UserDetails>, PersistenceError> {
    let mut rows = connection
        .query(
            &format!("SELECT {USER_COLUMNS} FROM users WHERE tenant_id = ?1 AND id = ?2"),
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(map_error)?;
    let Some(user_row) = rows.next().await.map_err(map_error)? else {
        return Ok(None);
    };
    let user = decode_user(&user_row)?;
    drop(rows);

    let mut role_rows = connection
        .query(
            "SELECT r.id, r.tenant_id, r.name, r.description, r.is_system, \
                    r.created_at, r.updated_at \
             FROM user_roles ur \
             JOIN roles r ON r.id = ur.role_id AND r.tenant_id = ur.tenant_id \
             WHERE ur.tenant_id = ?1 AND ur.user_id = ?2 \
             ORDER BY r.name COLLATE BINARY ASC, r.id ASC",
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(map_error)?;
    let mut roles = Vec::new();
    while let Some(role_record) = role_rows.next().await.map_err(map_error)? {
        roles.push(decode_role(&role_record)?);
    }
    drop(role_rows);

    let mut permission_rows = connection
        .query(
            "SELECT DISTINCT rp.permission FROM user_roles ur \
             JOIN role_permissions rp ON rp.role_id = ur.role_id \
             WHERE ur.tenant_id = ?1 AND ur.user_id = ?2 \
             ORDER BY rp.permission COLLATE BINARY ASC",
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(map_error)?;
    let mut permissions = Vec::new();
    while let Some(permission) = permission_rows.next().await.map_err(map_error)? {
        permissions.push(permission.get(0).map_err(map_error)?);
    }
    Ok(Some(UserDetails {
        user,
        roles,
        permissions,
    }))
}

async fn selected_roles(
    connection: &Connection,
    tenant: &TenantId,
    requested: Option<Vec<i32>>,
) -> Result<Option<Vec<Role>>, PersistenceError> {
    let mut rows = connection
        .query(
            &format!(
                "SELECT {ROLE_COLUMNS} FROM roles WHERE tenant_id = ?1 \
                 ORDER BY name COLLATE BINARY ASC, id ASC"
            ),
            params![tenant.as_str()],
        )
        .await
        .map_err(map_error)?;
    let mut available = Vec::new();
    while let Some(role) = rows.next().await.map_err(map_error)? {
        available.push(decode_role(&role)?);
    }
    match requested {
        Some(ids) => {
            let ids = ids.into_iter().collect::<HashSet<_>>();
            let selected = available
                .into_iter()
                .filter(|role| ids.contains(&role.id))
                .collect::<Vec<_>>();
            Ok((selected.len() == ids.len()).then_some(selected))
        }
        None => Ok(available
            .into_iter()
            .find(|role| role.name == VIEWER_ROLE)
            .map(|role| vec![role])),
    }
}

fn primary_role(roles: &[Role]) -> &str {
    roles
        .iter()
        .find(|role| role.name == OWNER_ROLE)
        .or_else(|| roles.iter().find(|role| role.name == ADMIN_ROLE))
        .or_else(|| roles.first())
        .map_or(VIEWER_ROLE, |role| role.name.as_str())
}

async fn replace_roles(
    connection: &Connection,
    tenant: &TenantId,
    user_id: i32,
    roles: &[Role],
) -> Result<(), PersistenceError> {
    connection
        .execute(
            "DELETE FROM user_roles WHERE tenant_id = ?1 AND user_id = ?2",
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(map_error)?;
    let now = Utc::now().timestamp_micros();
    for role in roles {
        connection
            .execute(
                "INSERT INTO user_roles (user_id, role_id, tenant_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![i64::from(user_id), i64::from(role.id), tenant.as_str(), now],
            )
            .await
            .map_err(map_error)?;
    }
    connection
        .execute(
            "UPDATE users SET role = ?3, permission_version = permission_version + 1 \
             WHERE tenant_id = ?1 AND id = ?2",
            params![tenant.as_str(), i64::from(user_id), primary_role(roles)],
        )
        .await
        .map_err(map_error)?;
    Ok(())
}

async fn owner_state(
    connection: &Connection,
    tenant: &TenantId,
    user_id: i32,
) -> Result<(Option<i32>, bool, i64), PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT r.id, \
                    EXISTS(SELECT 1 FROM user_roles ur \
                           WHERE ur.tenant_id = ?1 AND ur.user_id = ?2 AND ur.role_id = r.id), \
                    (SELECT count(DISTINCT ur.user_id) FROM user_roles ur \
                     WHERE ur.tenant_id = ?1 AND ur.role_id = r.id) \
             FROM roles r WHERE r.tenant_id = ?1 AND r.name = 'owner'",
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(map_error)?;
    let Some(record) = rows.next().await.map_err(map_error)? else {
        return Ok((None, false, 0));
    };
    Ok((
        Some(row::i32(record.get(0).map_err(map_error)?, "roles.id")?),
        record.get::<i64>(1).map_err(map_error)? != 0,
        record.get(2).map_err(map_error)?,
    ))
}

/// Turso user adapter. Clones share one engine and its serialized writer.
#[derive(Clone)]
pub struct TursoUserRepository {
    handles: TursoConnectionHandles,
}

impl TursoUserRepository {
    #[must_use]
    pub fn new(database: TursoDatabase) -> Self {
        Self::from_handles(database.shared_handles())
    }

    #[must_use]
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
}

#[async_trait]
impl UserRepository for TursoUserRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        page: PageRequest,
    ) -> Result<UserPage, PersistenceError> {
        let connection = self.handles.connect()?;
        let mut count_rows = connection
            .query(
                "SELECT count(*) FROM users WHERE tenant_id = ?1",
                params![tenant.as_str()],
            )
            .await
            .map_err(map_error)?;
        let total = count_rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(map_error)?;
        drop(count_rows);
        let mut rows = connection
            .query(
                "SELECT id FROM users WHERE tenant_id = ?1 \
                 ORDER BY username COLLATE BINARY ASC, id ASC LIMIT ?2 OFFSET ?3",
                params![tenant.as_str(), page.limit(), page.offset()],
            )
            .await
            .map_err(map_error)?;
        let mut ids = Vec::new();
        while let Some(record) = rows.next().await.map_err(map_error)? {
            ids.push(row::i32(record.get(0).map_err(map_error)?, "users.id")?);
        }
        drop(rows);
        let mut records = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(details) = hydrate(&connection, tenant, id).await? {
                records.push(details);
            }
        }
        Ok(UserPage { records, total })
    }

    async fn create(
        &self,
        tenant: &TenantId,
        user: NewUser,
    ) -> Result<CreateUserOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        let Some(roles) = selected_roles(&transaction, tenant, user.role_ids).await? else {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(CreateUserOutcome::RolesNotFound);
        };
        transaction
            .execute(
                "INSERT INTO users (tenant_id, username, password_hash, role, \
                                    is_active, permission_version, created_at) \
                 VALUES (?1, ?2, ?3, ?4, 1, 1, ?5)",
                params![
                    tenant.as_str(),
                    user.username,
                    user.password_hash.into_inner(),
                    primary_role(&roles),
                    Utc::now().timestamp_micros()
                ],
            )
            .await
            .map_err(map_error)?;
        let user_id = row::i32(transaction.last_insert_rowid(), "users.id")?;
        replace_roles(&transaction, tenant, user_id, &roles).await?;
        let result = hydrate(&transaction, tenant, user_id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(CreateUserOutcome::Created(result))
    }

    async fn change_password(
        &self,
        tenant: &TenantId,
        user_id: i32,
        password_hash: EncodedPasswordHash,
    ) -> Result<ChangePasswordOutcome, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        let changed = writer
            .execute(
                "UPDATE users SET password_hash = ?3, \
                                  permission_version = permission_version + 1 \
                 WHERE tenant_id = ?1 AND id = ?2",
                params![
                    tenant.as_str(),
                    i64::from(user_id),
                    password_hash.into_inner()
                ],
            )
            .await
            .map_err(map_error)?;
        Ok(if changed == 0 {
            ChangePasswordOutcome::NotFound
        } else {
            ChangePasswordOutcome::PasswordChanged
        })
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<DeleteUserOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        if hydrate(&transaction, tenant, user_id).await?.is_none() {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(DeleteUserOutcome::NotFound);
        }
        let (_, is_owner, owner_count) = owner_state(&transaction, tenant, user_id).await?;
        if is_owner && owner_count <= 1 {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(DeleteUserOutcome::WouldDeleteLastOwner);
        }
        transaction
            .execute(
                "DELETE FROM users WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(user_id)],
            )
            .await
            .map_err(map_error)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(DeleteUserOutcome::Deleted)
    }

    async fn set_roles(
        &self,
        tenant: &TenantId,
        user_id: i32,
        role_ids: Vec<i32>,
    ) -> Result<SetUserRolesOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        if hydrate(&transaction, tenant, user_id).await?.is_none() {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(SetUserRolesOutcome::UserNotFound);
        }
        let Some(roles) = selected_roles(&transaction, tenant, Some(role_ids)).await? else {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(SetUserRolesOutcome::RolesNotFound);
        };
        let (owner_id, is_owner, owner_count) = owner_state(&transaction, tenant, user_id).await?;
        if is_owner
            && owner_count <= 1
            && !owner_id.is_some_and(|id| roles.iter().any(|role| role.id == id))
        {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(SetUserRolesOutcome::WouldRemoveLastOwner);
        }
        replace_roles(&transaction, tenant, user_id, &roles).await?;
        let result = hydrate(&transaction, tenant, user_id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(SetUserRolesOutcome::Updated(result))
    }

    async fn find_credentials_by_username(
        &self,
        tenant: &TenantId,
        username: &str,
    ) -> Result<Option<UserCredentials>, PersistenceError> {
        let connection = self.handles.connect()?;
        let mut rows = connection
            .query(
                "SELECT id, password_hash FROM users WHERE tenant_id = ?1 AND username = ?2",
                params![tenant.as_str(), username],
            )
            .await
            .map_err(map_error)?;
        let Some(record) = rows.next().await.map_err(map_error)? else {
            return Ok(None);
        };
        let id = row::i32(record.get(0).map_err(map_error)?, "users.id")?;
        let password_hash = EncodedPasswordHash::new(record.get::<String>(1).map_err(map_error)?);
        drop(rows);
        Ok(hydrate(&connection, tenant, id)
            .await?
            .map(|details| UserCredentials {
                details,
                password_hash,
            }))
    }

    async fn get_details(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<Option<UserDetails>, PersistenceError> {
        hydrate(&self.handles.connect()?, tenant, user_id).await
    }

    async fn record_successful_login(
        &self,
        tenant: &TenantId,
        user_id: i32,
        logged_in_at: DateTime<Utc>,
    ) -> Result<RecordSuccessfulLoginOutcome, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        let changed = writer
            .execute(
                "UPDATE users SET last_login_at = ?3 WHERE tenant_id = ?1 AND id = ?2",
                params![
                    tenant.as_str(),
                    i64::from(user_id),
                    logged_in_at.timestamp_micros()
                ],
            )
            .await
            .map_err(map_error)?;
        Ok(if changed == 0 {
            RecordSuccessfulLoginOutcome::NotFound
        } else {
            RecordSuccessfulLoginOutcome::LoginRecorded
        })
    }
}
