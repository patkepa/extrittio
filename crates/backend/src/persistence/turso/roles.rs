use async_trait::async_trait;
use chrono::Utc;
use turso::{Connection, Row, params};

use crate::domains::identity::role_repository::RoleRepository;
use crate::domains::identity::role_types::{
    CreateRoleRecord, DeleteRoleOutcome, RoleDetails, RoleRecord, UpdateRoleOutcome,
    UpdateRoleRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

fn decode_role(record: &Row) -> Result<RoleRecord, PersistenceError> {
    Ok(RoleRecord {
        id: row::i32(record.get::<i64>(0).map_err(row::error)?, "roles.id")?,
        name: record.get(1).map_err(row::error)?,
        description: record.get(2).map_err(row::error)?,
        is_system: record.get::<i64>(3).map_err(row::error)? != 0,
        created_at: row::datetime(record.get(4).map_err(row::error)?)?,
        updated_at: row::datetime(record.get(5).map_err(row::error)?)?,
    })
}

async fn details(
    connection: &Connection,
    tenant: &TenantId,
    id: i32,
) -> Result<Option<RoleDetails>, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT id, name, description, is_system, created_at, updated_at
             FROM roles WHERE tenant_id = ?1 AND id = ?2",
            params![tenant.as_str(), i64::from(id)],
        )
        .await
        .map_err(row::error)?;
    let Some(record) = rows.next().await.map_err(row::error)? else {
        return Ok(None);
    };
    let role = decode_role(&record)?;
    drop(rows);
    let mut permission_rows = connection
        .query(
            "SELECT permission FROM role_permissions WHERE role_id = ?1 ORDER BY permission",
            params![i64::from(id)],
        )
        .await
        .map_err(row::error)?;
    let mut permissions = Vec::new();
    while let Some(permission) = permission_rows.next().await.map_err(row::error)? {
        permissions.push(permission.get(0).map_err(row::error)?);
    }
    let mut count_rows = connection
        .query(
            "SELECT count(*) FROM user_roles WHERE tenant_id = ?1 AND role_id = ?2",
            params![tenant.as_str(), i64::from(id)],
        )
        .await
        .map_err(row::error)?;
    let user_count = count_rows
        .next()
        .await
        .map_err(row::error)?
        .ok_or(PersistenceError::NotFound)?
        .get(0)
        .map_err(row::error)?;
    Ok(Some(RoleDetails {
        role,
        permissions,
        user_count,
    }))
}

#[async_trait]
impl RoleRepository for TursoAdapter {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<RoleDetails>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut id_rows = connection
            .query(
                "SELECT id FROM roles WHERE tenant_id = ?1 ORDER BY name, id",
                params![tenant.as_str()],
            )
            .await
            .map_err(row::error)?;
        let mut ids = Vec::new();
        while let Some(record) = id_rows.next().await.map_err(row::error)? {
            ids.push(row::i32(record.get(0).map_err(row::error)?, "roles.id")?);
        }
        let mut records = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(record) = details(&connection, tenant, id).await? {
                records.push(record);
            }
        }
        Ok(records)
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateRoleRecord,
    ) -> Result<RoleDetails, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let now = Utc::now().timestamp_micros();
        transaction
            .execute(
                "INSERT INTO roles (tenant_id, name, description, is_system, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, ?4, ?4)",
                params![tenant.as_str(), record.name, record.description, now],
            )
            .await
            .map_err(row::error)?;
        let id = row::i32(transaction.last_insert_rowid(), "roles.id")?;
        for permission in record.permissions {
            transaction
                .execute(
                    "INSERT INTO role_permissions (role_id, permission) VALUES (?1, ?2)",
                    params![i64::from(id), permission],
                )
                .await
                .map_err(row::error)?;
        }
        let result = details(&transaction, tenant, id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        transaction.commit().await.map_err(row::error)?;
        Ok(result)
    }

    async fn update(
        &self,
        tenant: &TenantId,
        id: i32,
        record: UpdateRoleRecord,
    ) -> Result<UpdateRoleOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let Some(current) = details(&transaction, tenant, id).await? else {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(UpdateRoleOutcome::NotFound);
        };
        if current.role.is_system {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(UpdateRoleOutcome::SystemRole);
        }
        if let Some(name) = record.name {
            transaction
                .execute(
                    "UPDATE roles SET name = ?3, updated_at = ?4 WHERE tenant_id = ?1 AND id = ?2",
                    params![
                        tenant.as_str(),
                        i64::from(id),
                        name,
                        Utc::now().timestamp_micros()
                    ],
                )
                .await
                .map_err(row::error)?;
        }
        if let Some(description) = record.description {
            transaction
                .execute(
                    "UPDATE roles SET description = ?3, updated_at = ?4 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), i64::from(id), description, Utc::now().timestamp_micros()],
                )
                .await
                .map_err(row::error)?;
        }
        if let Some(permissions) = record.permissions {
            transaction
                .execute(
                    "DELETE FROM role_permissions WHERE role_id = ?1",
                    params![i64::from(id)],
                )
                .await
                .map_err(row::error)?;
            for permission in permissions {
                transaction
                    .execute(
                        "INSERT INTO role_permissions (role_id, permission) VALUES (?1, ?2)",
                        params![i64::from(id), permission],
                    )
                    .await
                    .map_err(row::error)?;
            }
        }
        let result = details(&transaction, tenant, id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        transaction.commit().await.map_err(row::error)?;
        Ok(UpdateRoleOutcome::Updated(result))
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<DeleteRoleOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let Some(current) = details(&transaction, tenant, id).await? else {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(DeleteRoleOutcome::NotFound);
        };
        if current.role.is_system {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(DeleteRoleOutcome::SystemRole);
        }
        if current.user_count > 0 {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(DeleteRoleOutcome::InUse {
                name: current.role.name,
                user_count: current.user_count,
            });
        }
        transaction
            .execute(
                "DELETE FROM roles WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(id)],
            )
            .await
            .map_err(row::error)?;
        transaction.commit().await.map_err(row::error)?;
        Ok(DeleteRoleOutcome::Deleted)
    }
}
