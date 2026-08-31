//! Turso implementation of tenant-scoped roles and permissions.

use async_trait::async_trait;
use chrono::Utc;
use extrittio_backend_core::{
    ConstraintName, DeleteRoleOutcome, NewRole, PersistenceError, Role, RoleDetails, RolePatch,
    RoleRepository, TenantId, UpdateRoleOutcome,
};
use turso::{Connection, Row, params};

use crate::error::map_error;
use crate::row;
use crate::{TursoConnectionHandles, TursoDatabase};

const ROLE_COLUMNS: &str = "id, tenant_id, name, description, is_system, created_at, updated_at";

#[derive(Clone)]
pub struct TursoRoleRepository {
    handles: TursoConnectionHandles,
}

impl TursoRoleRepository {
    #[must_use]
    pub fn new(database: TursoDatabase) -> Self {
        Self::from_handles(database.shared_handles())
    }

    #[must_use]
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
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

fn map_role_name_error(error: turso::Error) -> PersistenceError {
    match map_error(error) {
        PersistenceError::UniqueViolation { .. } => PersistenceError::UniqueViolation {
            constraint: ConstraintName::new("roles.tenant_name"),
        },
        other => other,
    }
}

async fn details(
    connection: &Connection,
    tenant: &TenantId,
    role_id: i32,
) -> Result<Option<RoleDetails>, PersistenceError> {
    let mut rows = connection
        .query(
            &format!("SELECT {ROLE_COLUMNS} FROM roles WHERE tenant_id = ?1 AND id = ?2"),
            params![tenant.as_str(), i64::from(role_id)],
        )
        .await
        .map_err(map_error)?;
    let Some(record) = rows.next().await.map_err(map_error)? else {
        return Ok(None);
    };
    let role = decode_role(&record)?;
    drop(rows);

    let mut permission_rows = connection
        .query(
            "SELECT permission FROM role_permissions WHERE role_id = ?1 \
             ORDER BY permission COLLATE BINARY ASC",
            params![i64::from(role_id)],
        )
        .await
        .map_err(map_error)?;
    let mut permissions = Vec::new();
    while let Some(permission) = permission_rows.next().await.map_err(map_error)? {
        permissions.push(permission.get(0).map_err(map_error)?);
    }
    drop(permission_rows);

    let mut count_rows = connection
        .query(
            "SELECT count(*) FROM user_roles WHERE tenant_id = ?1 AND role_id = ?2",
            params![tenant.as_str(), i64::from(role_id)],
        )
        .await
        .map_err(map_error)?;
    let user_count = count_rows
        .next()
        .await
        .map_err(map_error)?
        .ok_or(PersistenceError::NotFound)?
        .get(0)
        .map_err(map_error)?;
    Ok(Some(RoleDetails {
        role,
        permissions,
        user_count,
    }))
}

#[async_trait]
impl RoleRepository for TursoRoleRepository {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<RoleDetails>, PersistenceError> {
        let connection = self.handles.connect()?;
        let mut id_rows = connection
            .query(
                "SELECT id FROM roles WHERE tenant_id = ?1 \
                 ORDER BY is_system DESC, name COLLATE BINARY ASC, id ASC",
                params![tenant.as_str()],
            )
            .await
            .map_err(map_error)?;
        let mut ids = Vec::new();
        while let Some(record) = id_rows.next().await.map_err(map_error)? {
            ids.push(row::i32(record.get(0).map_err(map_error)?, "roles.id")?);
        }
        drop(id_rows);

        let mut records = Vec::with_capacity(ids.len());
        for role_id in ids {
            if let Some(record) = details(&connection, tenant, role_id).await? {
                records.push(record);
            }
        }
        Ok(records)
    }

    async fn create(
        &self,
        tenant: &TenantId,
        role: NewRole,
    ) -> Result<RoleDetails, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        let now = Utc::now().timestamp_micros();
        transaction
            .execute(
                "INSERT INTO roles (tenant_id, name, description, is_system, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, 0, ?4, ?4)",
                params![tenant.as_str(), role.name, role.description, now],
            )
            .await
            .map_err(map_role_name_error)?;
        let role_id = row::i32(transaction.last_insert_rowid(), "roles.id")?;
        for permission in role.permissions {
            transaction
                .execute(
                    "INSERT INTO role_permissions (role_id, permission) VALUES (?1, ?2)",
                    params![i64::from(role_id), permission],
                )
                .await
                .map_err(map_error)?;
        }
        let result = details(&transaction, tenant, role_id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(result)
    }

    async fn update(
        &self,
        tenant: &TenantId,
        role_id: i32,
        patch: RolePatch,
    ) -> Result<UpdateRoleOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        let Some(current) = details(&transaction, tenant, role_id).await? else {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(UpdateRoleOutcome::NotFound);
        };
        if current.role.is_system {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(UpdateRoleOutcome::SystemRole);
        }

        let now = Utc::now().timestamp_micros();
        transaction
            .execute(
                "UPDATE roles SET updated_at = ?3 WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(role_id), now],
            )
            .await
            .map_err(map_error)?;
        if let Some(name) = patch.name {
            transaction
                .execute(
                    "UPDATE roles SET name = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), i64::from(role_id), name],
                )
                .await
                .map_err(map_role_name_error)?;
        }
        if let Some(description) = patch.description {
            transaction
                .execute(
                    "UPDATE roles SET description = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), i64::from(role_id), description],
                )
                .await
                .map_err(map_error)?;
        }
        if let Some(permissions) = patch.permissions {
            transaction
                .execute(
                    "DELETE FROM role_permissions WHERE role_id = ?1",
                    params![i64::from(role_id)],
                )
                .await
                .map_err(map_error)?;
            for permission in permissions {
                transaction
                    .execute(
                        "INSERT INTO role_permissions (role_id, permission) VALUES (?1, ?2)",
                        params![i64::from(role_id), permission],
                    )
                    .await
                    .map_err(map_error)?;
            }
            transaction
                .execute(
                    "UPDATE users SET permission_version = permission_version + 1 \
                     WHERE tenant_id = ?1 AND id IN ( \
                         SELECT user_id FROM user_roles WHERE tenant_id = ?1 AND role_id = ?2 \
                     )",
                    params![tenant.as_str(), i64::from(role_id)],
                )
                .await
                .map_err(map_error)?;
        }

        let result = details(&transaction, tenant, role_id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(UpdateRoleOutcome::Updated(result))
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        role_id: i32,
    ) -> Result<DeleteRoleOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        let Some(current) = details(&transaction, tenant, role_id).await? else {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(DeleteRoleOutcome::NotFound);
        };
        if current.role.is_system {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(DeleteRoleOutcome::SystemRole);
        }
        if current.user_count > 0 {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(DeleteRoleOutcome::InUse {
                name: current.role.name,
                user_count: current.user_count,
            });
        }

        transaction
            .execute(
                "DELETE FROM roles WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(role_id)],
            )
            .await
            .map_err(map_error)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(DeleteRoleOutcome::Deleted)
    }
}
