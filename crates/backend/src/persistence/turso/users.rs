use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use extrittio_backend_core::{Role, TenantId as CoreTenantId};
use turso::{Connection, Row, params};

use crate::domains::identity::user_repository::UserRepository;
use crate::domains::identity::user_types::{
    CreateUserOutcome, CreateUserRecord, DeleteUserOutcome, SetUserRolesOutcome, UserCredentials,
    UserDetails, UserList, UserRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

fn decode_user(record: &Row) -> Result<(UserRecord, String), PersistenceError> {
    Ok((
        UserRecord {
            id: row::i32(record.get::<i64>(0).map_err(row::error)?, "users.id")?,
            tenant_id: record.get(1).map_err(row::error)?,
            username: record.get(2).map_err(row::error)?,
            role: record.get(3).map_err(row::error)?,
            created_at: row::datetime(record.get(5).map_err(row::error)?)?,
            is_active: record.get::<i64>(6).map_err(row::error)? != 0,
            permission_version: row::i32(
                record.get(7).map_err(row::error)?,
                "users.permission_version",
            )?,
            last_login_at: record
                .get::<Option<i64>>(8)
                .map_err(row::error)?
                .map(row::datetime)
                .transpose()?,
        },
        record.get(4).map_err(row::error)?,
    ))
}

fn decode_role(record: &Row) -> Result<Role, PersistenceError> {
    let tenant_id = CoreTenantId::new(record.get::<String>(1).map_err(row::error)?)
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
    Ok(Role {
        id: row::i32(record.get::<i64>(0).map_err(row::error)?, "roles.id")?,
        tenant_id,
        name: record.get(2).map_err(row::error)?,
        description: record.get(3).map_err(row::error)?,
        is_system: record.get::<i64>(4).map_err(row::error)? != 0,
        created_at: row::datetime(record.get(5).map_err(row::error)?)?,
        updated_at: row::datetime(record.get(6).map_err(row::error)?)?,
    })
}

async fn hydrate(
    connection: &Connection,
    tenant: &TenantId,
    user_id: i32,
) -> Result<Option<(UserDetails, String)>, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT id, tenant_id, username, role, password_hash, created_at,
                    is_active, permission_version, last_login_at
             FROM users WHERE tenant_id = ?1 AND id = ?2",
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(row::error)?;
    let Some(user_row) = rows.next().await.map_err(row::error)? else {
        return Ok(None);
    };
    let (user, password_hash) = decode_user(&user_row)?;
    drop(rows);
    let mut role_rows = connection
        .query(
            "SELECT r.id, r.tenant_id, r.name, r.description, r.is_system, r.created_at, r.updated_at
             FROM user_roles ur JOIN roles r ON r.id = ur.role_id AND r.tenant_id = ur.tenant_id
             WHERE ur.tenant_id = ?1 AND ur.user_id = ?2 ORDER BY r.name, r.id",
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(row::error)?;
    let mut roles = Vec::new();
    while let Some(role_record) = role_rows.next().await.map_err(row::error)? {
        roles.push(decode_role(&role_record)?);
    }
    let mut permission_rows = connection
        .query(
            "SELECT DISTINCT rp.permission FROM user_roles ur
             JOIN role_permissions rp ON rp.role_id = ur.role_id
             WHERE ur.tenant_id = ?1 AND ur.user_id = ?2 ORDER BY rp.permission",
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(row::error)?;
    let mut permissions = Vec::new();
    while let Some(permission) = permission_rows.next().await.map_err(row::error)? {
        permissions.push(permission.get(0).map_err(row::error)?);
    }
    Ok(Some((
        UserDetails {
            user,
            roles,
            permissions,
        },
        password_hash,
    )))
}

async fn selected_roles(
    connection: &Connection,
    tenant: &TenantId,
    requested: Option<Vec<i32>>,
) -> Result<Option<Vec<Role>>, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT id, tenant_id, name, description, is_system, created_at, updated_at
             FROM roles WHERE tenant_id = ?1 ORDER BY name, id",
            params![tenant.as_str()],
        )
        .await
        .map_err(row::error)?;
    let mut available = Vec::new();
    while let Some(role) = rows.next().await.map_err(row::error)? {
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
            .find(|role| role.name == "viewer")
            .map(|role| vec![role])),
    }
}

fn primary_role(roles: &[Role]) -> &str {
    roles
        .iter()
        .find(|role| role.name == "owner")
        .or_else(|| roles.iter().find(|role| role.name == "admin"))
        .or_else(|| roles.first())
        .map_or("viewer", |role| role.name.as_str())
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
        .map_err(row::error)?;
    let now = Utc::now().timestamp_micros();
    for role in roles {
        connection
            .execute(
                "INSERT INTO user_roles (user_id, role_id, tenant_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![i64::from(user_id), i64::from(role.id), tenant.as_str(), now],
            )
            .await
            .map_err(row::error)?;
    }
    connection
        .execute(
            "UPDATE users SET role = ?3, permission_version = permission_version + 1
             WHERE tenant_id = ?1 AND id = ?2",
            params![tenant.as_str(), i64::from(user_id), primary_role(roles)],
        )
        .await
        .map_err(row::error)?;
    Ok(())
}

async fn owner_state(
    connection: &Connection,
    tenant: &TenantId,
    user_id: i32,
) -> Result<(Option<i32>, bool, i64), PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT r.id,
                    EXISTS(SELECT 1 FROM user_roles ur WHERE ur.tenant_id = ?1 AND ur.user_id = ?2 AND ur.role_id = r.id),
                    (SELECT count(DISTINCT ur.user_id) FROM user_roles ur WHERE ur.tenant_id = ?1 AND ur.role_id = r.id)
             FROM roles r WHERE r.tenant_id = ?1 AND r.name = 'owner'",
            params![tenant.as_str(), i64::from(user_id)],
        )
        .await
        .map_err(row::error)?;
    let Some(record) = rows.next().await.map_err(row::error)? else {
        return Ok((None, false, 0));
    };
    Ok((
        Some(row::i32(record.get(0).map_err(row::error)?, "roles.id")?),
        record.get::<i64>(1).map_err(row::error)? != 0,
        record.get(2).map_err(row::error)?,
    ))
}

#[async_trait]
impl UserRepository for TursoAdapter {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<UserList, PersistenceError> {
        let connection = self.database.connect()?;
        let mut count_rows = connection
            .query(
                "SELECT count(*) FROM users WHERE tenant_id = ?1",
                params![tenant.as_str()],
            )
            .await
            .map_err(row::error)?;
        let total = count_rows
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::error)?;
        let mut rows = connection
            .query(
                "SELECT id FROM users WHERE tenant_id = ?1 ORDER BY username, id LIMIT ?2 OFFSET ?3",
                params![tenant.as_str(), limit, offset],
            )
            .await
            .map_err(row::error)?;
        let mut ids = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            ids.push(row::i32(record.get(0).map_err(row::error)?, "users.id")?);
        }
        let mut records = Vec::new();
        for id in ids {
            if let Some((details, _)) = hydrate(&connection, tenant, id).await? {
                records.push(details);
            }
        }
        Ok(UserList { records, total })
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateUserRecord,
    ) -> Result<CreateUserOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let Some(roles) = selected_roles(&transaction, tenant, record.role_ids).await? else {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(CreateUserOutcome::RolesNotFound);
        };
        transaction
            .execute(
                "INSERT INTO users (tenant_id, username, password_hash, role, is_active, permission_version, created_at)
                 VALUES (?1, ?2, ?3, ?4, 1, 1, ?5)",
                params![tenant.as_str(), record.username, record.password_hash, primary_role(&roles), Utc::now().timestamp_micros()],
            )
            .await
            .map_err(row::error)?;
        let id = row::i32(transaction.last_insert_rowid(), "users.id")?;
        replace_roles(&transaction, tenant, id, &roles).await?;
        let result = hydrate(&transaction, tenant, id)
            .await?
            .ok_or(PersistenceError::NotFound)?
            .0;
        transaction.commit().await.map_err(row::error)?;
        Ok(CreateUserOutcome::Created(result))
    }

    async fn change_password(
        &self,
        tenant: &TenantId,
        user_id: i32,
        password_hash: String,
    ) -> Result<bool, PersistenceError> {
        let writer = self.database.writer().await;
        writer.execute("UPDATE users SET password_hash = ?3, permission_version = permission_version + 1 WHERE tenant_id = ?1 AND id = ?2", params![tenant.as_str(), i64::from(user_id), password_hash]).await.map(|count| count > 0).map_err(row::error)
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<DeleteUserOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        if hydrate(&transaction, tenant, user_id).await?.is_none() {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(DeleteUserOutcome::NotFound);
        }
        let (_, is_owner, owner_count) = owner_state(&transaction, tenant, user_id).await?;
        if is_owner && owner_count <= 1 {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(DeleteUserOutcome::WouldDeleteLastOwner);
        }
        transaction
            .execute(
                "DELETE FROM users WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(user_id)],
            )
            .await
            .map_err(row::error)?;
        transaction.commit().await.map_err(row::error)?;
        Ok(DeleteUserOutcome::Deleted)
    }

    async fn set_roles(
        &self,
        tenant: &TenantId,
        user_id: i32,
        role_ids: Vec<i32>,
    ) -> Result<SetUserRolesOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        if hydrate(&transaction, tenant, user_id).await?.is_none() {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(SetUserRolesOutcome::UserNotFound);
        }
        let Some(roles) = selected_roles(&transaction, tenant, Some(role_ids)).await? else {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(SetUserRolesOutcome::RolesNotFound);
        };
        let (owner_id, is_owner, owner_count) = owner_state(&transaction, tenant, user_id).await?;
        if is_owner
            && owner_count <= 1
            && !owner_id.is_some_and(|id| roles.iter().any(|role| role.id == id))
        {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(SetUserRolesOutcome::WouldRemoveLastOwner);
        }
        replace_roles(&transaction, tenant, user_id, &roles).await?;
        let result = hydrate(&transaction, tenant, user_id)
            .await?
            .ok_or(PersistenceError::NotFound)?
            .0;
        transaction.commit().await.map_err(row::error)?;
        Ok(SetUserRolesOutcome::Updated(result))
    }

    async fn find_credentials_by_username(
        &self,
        tenant: &TenantId,
        username: &str,
    ) -> Result<Option<UserCredentials>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection
            .query(
                "SELECT id FROM users WHERE tenant_id = ?1 AND username = ?2",
                params![tenant.as_str(), username],
            )
            .await
            .map_err(row::error)?;
        let Some(record) = rows.next().await.map_err(row::error)? else {
            return Ok(None);
        };
        let id = row::i32(record.get(0).map_err(row::error)?, "users.id")?;
        drop(rows);
        Ok(hydrate(&connection, tenant, id)
            .await?
            .map(|(details, password_hash)| UserCredentials {
                details,
                password_hash,
            }))
    }

    async fn get_details(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<Option<UserDetails>, PersistenceError> {
        Ok(hydrate(&self.database.connect()?, tenant, user_id)
            .await?
            .map(|value| value.0))
    }

    async fn record_successful_login(
        &self,
        tenant: &TenantId,
        user_id: i32,
        logged_in_at: DateTime<Utc>,
    ) -> Result<bool, PersistenceError> {
        let writer = self.database.writer().await;
        writer
            .execute(
                "UPDATE users SET last_login_at = ?3 WHERE tenant_id = ?1 AND id = ?2",
                params![
                    tenant.as_str(),
                    i64::from(user_id),
                    logged_in_at.timestamp_micros()
                ],
            )
            .await
            .map(|count| count > 0)
            .map_err(row::error)
    }
}
