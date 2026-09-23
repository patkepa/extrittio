use async_trait::async_trait;
use chrono::Utc;
use turso::params;

use extrittio_backend_core::TenantId;
use extrittio_backend_core::bootstrap::{BootstrapOwner, BootstrapRepository, SeedOwnerOutcome};
use extrittio_backend_core::{Permission, PersistenceError};

use crate::TursoConnectionHandles;
#[derive(Clone)]
pub struct TursoBootstrapRepository {
    handles: TursoConnectionHandles,
}
impl TursoBootstrapRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}

#[async_trait]
impl BootstrapRepository for TursoBootstrapRepository {
    async fn get_or_create_server_config(
        &self,
        key: &str,
        generated_value: String,
    ) -> Result<String, PersistenceError> {
        let writer = self.handles.lock_writer().await;
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
        let connection = self.connect()?;
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
        let mut writer = self.handles.lock_writer().await;
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
        let auth_epoch = uuid::Uuid::new_v4().to_string();
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
                                    permission_version, auth_epoch, created_at)
                 VALUES (?1, ?2, ?3, 'owner', 1, 2, ?4, ?5)",
                params![
                    tenant.as_str(),
                    owner.username,
                    owner.password_hash.into_inner(),
                    auth_epoch,
                    now
                ],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bootstrap_preserves_owner_and_secret_without_device_type_seeding() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open_and_migrate(
            directory.path(),
            &directory.path().join("bootstrap.db"),
            std::time::Duration::from_secs(1),
        )
        .await
        .unwrap();
        let repository = TursoBootstrapRepository::from_handles(database.shared_handles());
        let tenant = TenantId::new("default").unwrap();
        assert!(!repository.users_exist().await.unwrap());
        assert_eq!(
            repository
                .get_or_create_server_config("jwt_secret", "first".into())
                .await
                .unwrap(),
            "first"
        );
        assert_eq!(
            repository
                .get_or_create_server_config("jwt_secret", "second".into())
                .await
                .unwrap(),
            "first"
        );
        let owner = BootstrapOwner {
            username: "operator".into(),
            password_hash: extrittio_backend_core::EncodedPasswordHash::new("test-only-verifier"),
        };
        assert_eq!(
            repository
                .seed_owner_if_empty(&tenant, owner.clone())
                .await
                .unwrap(),
            SeedOwnerOutcome::Created
        );
        assert_eq!(
            repository
                .seed_owner_if_empty(&tenant, owner)
                .await
                .unwrap(),
            SeedOwnerOutcome::SkippedUsersExist
        );
        assert!(repository.users_exist().await.unwrap());
        let connection = database.shared_handles().connect().unwrap();
        let mut rows = connection
            .query(
                "SELECT u.username,u.auth_epoch,r.name FROM users u
             JOIN user_roles ur ON ur.tenant_id=u.tenant_id AND ur.user_id=u.id
             JOIN roles r ON r.tenant_id=ur.tenant_id AND r.id=ur.role_id
             WHERE u.tenant_id='default'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "operator");
        assert!(!row.get::<String>(1).unwrap().is_empty());
        assert_eq!(row.get::<String>(2).unwrap(), "owner");
        assert!(rows.next().await.unwrap().is_none());
        drop(rows);
        let mut rows = connection
            .query(
                "SELECT (SELECT count(*) FROM device_blueprints),
                    (SELECT count(*) FROM sqlite_schema WHERE name='device_types')",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<i64>(0).unwrap(), 0);
        assert_eq!(row.get::<i64>(1).unwrap(), 0);
    }
}
