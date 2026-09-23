use async_trait::async_trait;
use diesel::prelude::*;

use crate::models::{ApiKey, NewApiKey};
use crate::schema::{api_keys, device_blueprints};
use extrittio_backend_core::ApiKeyRepository;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::{ApiKeyRecord, ApiKeySummary, CreateApiKeyRecord};

use crate::error::map_diesel_error;
use crate::{PostgresExecutor, PostgresPool};

#[derive(Clone)]
pub struct PostgresApiKeyRepository {
    executor: PostgresExecutor,
}

impl PostgresApiKeyRepository {
    #[must_use]
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}

fn to_record(row: ApiKey) -> ApiKeyRecord {
    ApiKeyRecord {
        id: row.id,
        name: row.name,
        key_prefix: row.key_prefix,
        blueprint_id: row.blueprint_id,
        created_at: row.created_at.and_utc(),
        last_used_at: row.last_used_at.map(|timestamp| timestamp.and_utc()),
    }
}

#[async_trait]
impl ApiKeyRepository for PostgresApiKeyRepository {
    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateApiKeyRecord,
    ) -> Result<ApiKeyRecord, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                if let Some(id) = record.blueprint_id.as_deref() {
                    let exists = diesel::select(diesel::dsl::exists(
                        device_blueprints::table
                            .filter(device_blueprints::tenant_id.eq(&tenant_id))
                            .filter(device_blueprints::id.eq(id)),
                    ))
                    .get_result::<bool>(connection)
                    .map_err(map_diesel_error)?;
                    if !exists {
                        return Err(PersistenceError::NotFound);
                    }
                }
                diesel::insert_into(api_keys::table)
                    .values(NewApiKey {
                        tenant_id,
                        name: record.name,
                        key_hash: record.key_hash,
                        key_prefix: record.key_prefix,
                        blueprint_id: record.blueprint_id,
                    })
                    .returning(ApiKey::as_returning())
                    .get_result(connection)
                    .map(to_record)
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn list(&self, tenant: &TenantId) -> Result<Vec<ApiKeySummary>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                api_keys::table
                    .left_outer_join(
                        device_blueprints::table.on(api_keys::blueprint_id
                            .eq(device_blueprints::id.nullable())
                            .and(api_keys::tenant_id.eq(device_blueprints::tenant_id))),
                    )
                    .filter(api_keys::tenant_id.eq(tenant_id))
                    .select((ApiKey::as_select(), device_blueprints::name.nullable()))
                    .order((api_keys::created_at.desc(), api_keys::id.desc()))
                    .load::<(ApiKey, Option<String>)>(connection)
                    .map(|rows| {
                        rows.into_iter()
                            .map(|(key, blueprint_name)| ApiKeySummary {
                                key: to_record(key),
                                blueprint_name,
                            })
                            .collect()
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::delete(
                    api_keys::table
                        .filter(api_keys::tenant_id.eq(tenant_id))
                        .filter(api_keys::id.eq(id)),
                )
                .execute(connection)
                .map(|rows| rows > 0)
                .map_err(map_diesel_error)
            })
            .await
    }
}
