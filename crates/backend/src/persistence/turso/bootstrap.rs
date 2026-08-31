use async_trait::async_trait;
use chrono::Utc;
use turso::params;

use crate::auth::policy::Permission;
use crate::persistence::{
    BootstrapOwner, BootstrapRepository, BuiltinDeviceType, PersistenceError, SeedOwnerOutcome,
};
use crate::tenancy::TenantId;

use super::TursoAdapter;

#[async_trait]
impl BootstrapRepository for TursoAdapter {
    async fn seed_builtin_device_types(
        &self,
        tenant: &TenantId,
        records: Vec<BuiltinDeviceType>,
    ) -> Result<(), PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        let now = Utc::now().timestamp_micros();
        for record in records {
            transaction
                .execute(
                    "INSERT INTO device_types (tenant_id, name, icon, color_hex, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT (tenant_id, name) DO NOTHING",
                    params![
                        tenant.as_str(),
                        record.name,
                        record.icon,
                        record.color_hex,
                        now
                    ],
                )
                .await
                .map_err(map_error)?;
        }
        transaction.commit().await.map_err(map_error)
    }

    async fn get_or_create_server_config(
        &self,
        key: &str,
        generated_value: String,
    ) -> Result<String, PersistenceError> {
        let writer = self.database.writer().await;
        writer
            .execute(
                "INSERT INTO server_config (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO NOTHING",
                params![key, generated_value],
            )
            .await
            .map_err(map_error)?;
        let mut rows = writer
            .query(
                "SELECT value FROM server_config WHERE key = ?1",
                params![key],
            )
            .await
            .map_err(map_error)?;
        let row = rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?;
        row.get(0).map_err(map_error)
    }

    async fn users_exist(&self) -> Result<bool, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection
            .query("SELECT EXISTS(SELECT 1 FROM users)", ())
            .await
            .map_err(map_error)?;
        let row = rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?;
        Ok(row.get::<i64>(0).map_err(map_error)? != 0)
    }

    async fn seed_owner_if_empty(
        &self,
        tenant: &TenantId,
        owner: BootstrapOwner,
    ) -> Result<SeedOwnerOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        let mut rows = transaction
            .query("SELECT EXISTS(SELECT 1 FROM users)", ())
            .await
            .map_err(map_error)?;
        let exists = rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(map_error)?
            != 0;
        drop(rows);
        if exists {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(SeedOwnerOutcome::SkippedUsersExist);
        }
        let now = Utc::now().timestamp_micros();
        transaction
            .execute(
                "INSERT INTO roles (tenant_id, name, description, is_system, created_at, updated_at)
                 VALUES (?1, 'owner', 'Full tenant owner with all permissions and lockout protection.', 1, ?2, ?2)
                 ON CONFLICT (tenant_id, name) DO NOTHING",
                params![tenant.as_str(), now],
            )
            .await
            .map_err(map_error)?;
        let mut role_rows = transaction
            .query(
                "SELECT id FROM roles WHERE tenant_id = ?1 AND name = 'owner'",
                params![tenant.as_str()],
            )
            .await
            .map_err(map_error)?;
        let role_id = role_rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(map_error)?;
        drop(role_rows);
        for permission in Permission::all() {
            transaction
                .execute(
                    "INSERT INTO role_permissions (role_id, permission) VALUES (?1, ?2)
                     ON CONFLICT DO NOTHING",
                    params![role_id, permission.key()],
                )
                .await
                .map_err(map_error)?;
        }
        transaction
            .execute(
                "INSERT INTO users (tenant_id, username, password_hash, role, is_active,
                                    permission_version, created_at)
                 VALUES (?1, ?2, ?3, 'owner', 1, 2, ?4)",
                params![tenant.as_str(), owner.username, owner.password_hash, now],
            )
            .await
            .map_err(map_error)?;
        let user_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO user_roles (user_id, role_id, tenant_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![user_id, role_id, tenant.as_str(), now],
            )
            .await
            .map_err(map_error)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(SeedOwnerOutcome::Created)
    }
}

fn map_error(error: turso::Error) -> PersistenceError {
    PersistenceError::Internal(error.to_string())
}
