use async_trait::async_trait;
use diesel::prelude::*;

use crate::db::models::{ApiKey, NewApiKey};
use crate::db::schema::{api_keys, device_types};
use crate::domains::identity::api_key_repository::ApiKeyRepository;
use crate::domains::identity::api_key_types::{ApiKeyRecord, ApiKeySummary, CreateApiKeyRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

fn to_record(row: ApiKey) -> ApiKeyRecord {
    ApiKeyRecord {
        id: row.id,
        name: row.name,
        key_prefix: row.key_prefix,
        device_type_id: row.device_type_id,
        created_at: row.created_at.and_utc(),
        last_used_at: row.last_used_at.map(|timestamp| timestamp.and_utc()),
    }
}

#[async_trait]
impl ApiKeyRepository for PostgresAdapter {
    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateApiKeyRecord,
    ) -> Result<ApiKeyRecord, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::insert_into(api_keys::table)
                    .values(NewApiKey {
                        tenant_id,
                        name: record.name,
                        key_hash: record.key_hash,
                        key_prefix: record.key_prefix,
                        device_type_id: record.device_type_id,
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
                        device_types::table.on(api_keys::device_type_id
                            .eq(device_types::id.nullable())
                            .and(api_keys::tenant_id.eq(device_types::tenant_id))),
                    )
                    .filter(api_keys::tenant_id.eq(tenant_id))
                    .select((ApiKey::as_select(), device_types::name.nullable()))
                    .order((api_keys::created_at.desc(), api_keys::id.desc()))
                    .load::<(ApiKey, Option<String>)>(connection)
                    .map(|rows| {
                        rows.into_iter()
                            .map(|(key, device_type_name)| ApiKeySummary {
                                key: to_record(key),
                                device_type_name,
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
