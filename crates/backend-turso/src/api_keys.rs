use async_trait::async_trait;
use chrono::Utc;
use turso::{Row, params};

use extrittio_backend_core::ApiKeyRepository;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::{ApiKeyRecord, ApiKeySummary, CreateApiKeyRecord};

use crate::row::legacy_error as map_error;
use crate::{TursoConnectionHandles, row};

#[derive(Clone)]
pub struct TursoApiKeyRepository {
    handles: TursoConnectionHandles,
}

impl TursoApiKeyRepository {
    #[must_use]
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
}

fn decode(record: &Row) -> Result<ApiKeyRecord, PersistenceError> {
    Ok(ApiKeyRecord {
        id: row::i32(record.get::<i64>(0).map_err(map_error)?, "api_keys.id")?,
        name: record.get(1).map_err(map_error)?,
        key_prefix: record.get(2).map_err(map_error)?,
        device_type_id: record
            .get::<Option<i64>>(3)
            .map_err(map_error)?
            .map(|value| row::i32(value, "api_keys.device_type_id"))
            .transpose()?,
        created_at: row::datetime(record.get(4).map_err(map_error)?)?,
        last_used_at: record
            .get::<Option<i64>>(5)
            .map_err(map_error)?
            .map(row::datetime)
            .transpose()?,
    })
}

#[async_trait]
impl ApiKeyRepository for TursoApiKeyRepository {
    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateApiKeyRecord,
    ) -> Result<ApiKeyRecord, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        let mut rows = writer
            .query(
                "INSERT INTO api_keys
                   (tenant_id, name, key_hash, key_prefix, device_type_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 RETURNING id, name, key_prefix, device_type_id, created_at, last_used_at",
                params![
                    tenant.as_str(),
                    record.name,
                    record.key_hash,
                    record.key_prefix,
                    record.device_type_id.map(i64::from),
                    Utc::now().timestamp_micros()
                ],
            )
            .await
            .map_err(map_error)?;
        decode(
            &rows
                .next()
                .await
                .map_err(map_error)?
                .ok_or(PersistenceError::NotFound)?,
        )
    }

    async fn list(&self, tenant: &TenantId) -> Result<Vec<ApiKeySummary>, PersistenceError> {
        let connection = self
            .handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let mut rows = connection
            .query(
                "SELECT a.id, a.name, a.key_prefix, a.device_type_id, a.created_at,
                        a.last_used_at, d.name
                 FROM api_keys a LEFT JOIN device_types d
                   ON d.tenant_id = a.tenant_id AND d.id = a.device_type_id
                 WHERE a.tenant_id = ?1 ORDER BY a.created_at DESC, a.id DESC",
                params![tenant.as_str()],
            )
            .await
            .map_err(map_error)?;
        let mut result = Vec::new();
        while let Some(record) = rows.next().await.map_err(map_error)? {
            result.push(ApiKeySummary {
                key: decode(&record)?,
                device_type_name: record.get(6).map_err(map_error)?,
            });
        }
        Ok(result)
    }

    async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer
            .execute(
                "DELETE FROM api_keys WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(id)],
            )
            .await
            .map(|count| count > 0)
            .map_err(map_error)
    }
}
