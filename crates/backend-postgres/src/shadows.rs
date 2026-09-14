use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::Connection;
use diesel::PgConnection;
use diesel::prelude::*;
use serde_json::{Map, Value};

use crate::models::{DeviceShadow, UpdateShadow};
use crate::schema::device_shadows;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::shadows::ShadowRepository;
use extrittio_backend_core::shadows::{
    ShadowMutationError, ShadowRecord, apply_desired_patch, apply_reported_patch, reset_shadow,
};

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresShadowRepository {
    executor: PostgresExecutor,
}
impl PostgresShadowRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
use crate::error::map_diesel_error;

fn to_record(row: DeviceShadow) -> ShadowRecord {
    ShadowRecord {
        device_id: row.device_id,
        desired: row.desired,
        reported: row.reported,
        delta: row.delta,
        version: row.version,
        updated_at: row.updated_at.and_utc(),
    }
}

fn map_mutation_error(error: ShadowMutationError) -> PersistenceError {
    PersistenceError::CorruptData(error.to_string())
}

enum ShadowTransactionError {
    Diesel(diesel::result::Error),
    Persistence(PersistenceError),
}

impl From<diesel::result::Error> for ShadowTransactionError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Diesel(error)
    }
}

impl From<PersistenceError> for ShadowTransactionError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

fn mutate_shadow(
    connection: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
    mutate: impl FnOnce(ShadowRecord) -> Result<ShadowRecord, ShadowMutationError>,
) -> Result<Option<ShadowRecord>, PersistenceError> {
    connection
        .transaction::<_, ShadowTransactionError, _>(|connection| {
            let row = device_shadows::table
                .filter(device_shadows::tenant_id.eq(tenant_id))
                .filter(device_shadows::device_id.eq(device_id))
                .select(DeviceShadow::as_select())
                .for_update()
                .first::<DeviceShadow>(connection)
                .optional()
                .map_err(map_diesel_error)?;
            let Some(row) = row else {
                return Ok(None);
            };
            let updated = mutate(to_record(row)).map_err(map_mutation_error)?;
            let row = diesel::update(
                device_shadows::table
                    .filter(device_shadows::tenant_id.eq(tenant_id))
                    .filter(device_shadows::device_id.eq(device_id)),
            )
            .set(UpdateShadow {
                desired: Some(updated.desired),
                reported: Some(updated.reported),
                delta: Some(updated.delta),
                version: Some(updated.version),
                updated_at: Some(updated.updated_at.naive_utc()),
            })
            .returning(DeviceShadow::as_returning())
            .get_result::<DeviceShadow>(connection)
            .map_err(map_diesel_error)?;
            Ok(Some(to_record(row)))
        })
        .map_err(|error| match error {
            ShadowTransactionError::Diesel(error) => map_diesel_error(error),
            ShadowTransactionError::Persistence(error) => error,
        })
}

#[async_trait]
impl ShadowRepository for PostgresShadowRepository {
    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<ShadowRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                device_shadows::table
                    .filter(device_shadows::tenant_id.eq(tenant_id))
                    .filter(device_shadows::device_id.eq(device_id))
                    .select(DeviceShadow::as_select())
                    .first::<DeviceShadow>(connection)
                    .optional()
                    .map(|row| row.map(to_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn update_desired(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShadowRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                mutate_shadow(connection, &tenant_id, &device_id, |shadow| {
                    apply_desired_patch(shadow, &patch, updated_at)
                })
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
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                mutate_shadow(connection, &tenant_id, &device_id, |shadow| {
                    apply_reported_patch(shadow, &patch, updated_at)
                })
            })
            .await
    }

    async fn reset(
        &self,
        tenant: &TenantId,
        device_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                mutate_shadow(connection, &tenant_id, &device_id, |shadow| {
                    reset_shadow(shadow, updated_at)
                })
                .map(|shadow| shadow.is_some())
            })
            .await
    }
}
