use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use turso::{Connection, Row, params};

use crate::domains::shadows::repository::ShadowRepository;
use crate::domains::shadows::types::{
    ShadowRecord, apply_desired_patch, apply_reported_patch, reset_shadow,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

fn decode(record: &Row) -> Result<ShadowRecord, PersistenceError> {
    let json = |index| -> Result<Value, PersistenceError> {
        let value: String = record.get(index).map_err(row::error)?;
        serde_json::from_str(&value)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))
    };
    Ok(ShadowRecord {
        device_id: record.get(0).map_err(row::error)?,
        desired: json(1)?,
        reported: json(2)?,
        delta: json(3)?,
        version: row::i32(record.get(4).map_err(row::error)?, "device_shadows.version")?,
        updated_at: row::datetime(record.get(5).map_err(row::error)?)?,
    })
}

async fn get_from(
    connection: &Connection,
    tenant: &TenantId,
    device_id: &str,
) -> Result<Option<ShadowRecord>, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT device_id, desired, reported, delta, version, updated_at
             FROM device_shadows WHERE tenant_id = ?1 AND device_id = ?2",
            params![tenant.as_str(), device_id],
        )
        .await
        .map_err(row::error)?;
    rows.next()
        .await
        .map_err(row::error)?
        .map(|record| decode(&record))
        .transpose()
}

async fn store(
    connection: &Connection,
    tenant: &TenantId,
    shadow: &ShadowRecord,
) -> Result<(), PersistenceError> {
    connection
        .execute(
            "UPDATE device_shadows SET desired = ?3, reported = ?4, delta = ?5,
                    version = ?6, updated_at = ?7 WHERE tenant_id = ?1 AND device_id = ?2",
            params![
                tenant.as_str(),
                shadow.device_id.clone(),
                serde_json::to_string(&shadow.desired)
                    .map_err(|error| PersistenceError::Internal(error.to_string()))?,
                serde_json::to_string(&shadow.reported)
                    .map_err(|error| PersistenceError::Internal(error.to_string()))?,
                serde_json::to_string(&shadow.delta)
                    .map_err(|error| PersistenceError::Internal(error.to_string()))?,
                i64::from(shadow.version),
                shadow.updated_at.timestamp_micros()
            ],
        )
        .await
        .map(|_| ())
        .map_err(row::error)
}

#[async_trait]
impl ShadowRepository for TursoAdapter {
    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<ShadowRecord>, PersistenceError> {
        get_from(&self.database.connect()?, tenant, device_id).await
    }

    async fn update_desired(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShadowRecord>, PersistenceError> {
        self.mutate(tenant, device_id, |shadow| {
            apply_desired_patch(shadow, &patch, updated_at)
        })
        .await
    }

    async fn update_reported(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShadowRecord>, PersistenceError> {
        self.mutate(tenant, device_id, |shadow| {
            apply_reported_patch(shadow, &patch, updated_at)
        })
        .await
    }

    async fn reset(
        &self,
        tenant: &TenantId,
        device_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, PersistenceError> {
        Ok(self
            .mutate(tenant, device_id, |shadow| reset_shadow(shadow, updated_at))
            .await?
            .is_some())
    }
}

impl TursoAdapter {
    async fn mutate(
        &self,
        tenant: &TenantId,
        device_id: &str,
        mutate: impl FnOnce(
            ShadowRecord,
        )
            -> Result<ShadowRecord, crate::domains::shadows::types::ShadowMutationError>,
    ) -> Result<Option<ShadowRecord>, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let Some(current) = get_from(&transaction, tenant, device_id).await? else {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(None);
        };
        let updated =
            mutate(current).map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
        store(&transaction, tenant, &updated).await?;
        transaction.commit().await.map_err(row::error)?;
        Ok(Some(updated))
    }
}
