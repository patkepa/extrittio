use async_trait::async_trait;
use diesel::prelude::*;

use crate::db::models::{CommandRecord as PgCommandRecord, NewCommandRecord as PgNewCommandRecord};
use crate::db::schema::{command_history, devices};
use crate::domains::commands::port::CommandRepository;
use crate::domains::commands::types::{CommandQuery, CommandRecord, NewCommandRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::PostgresAdapter;
use super::executor::map_diesel_error;

const TERMINAL_STATUSES: &[&str] = &["succeeded", "failed", "timed_out"];

fn to_record(record: PgCommandRecord) -> CommandRecord {
    CommandRecord {
        id: record.id,
        device_id: record.device_id,
        command: record.command,
        params: record.params,
        status: record.status,
        response_payload: record.response_payload,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

#[async_trait]
impl CommandRepository for PostgresAdapter {
    async fn create(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: NewCommandRecord,
    ) -> Result<Option<CommandRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                let exists = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .filter(devices::id.eq(&device_id))
                    .select(devices::id)
                    .first::<String>(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                    .is_some();
                if !exists {
                    return Ok(None);
                }
                let inserted = diesel::insert_into(command_history::table)
                    .values(PgNewCommandRecord {
                        id: record.id,
                        tenant_id,
                        device_id,
                        command: record.command,
                        params: record.params,
                    })
                    .returning(PgCommandRecord::as_returning())
                    .get_result::<PgCommandRecord>(connection)
                    .map_err(map_diesel_error)?;
                Ok(Some(to_record(inserted)))
            })
            .await
    }

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: CommandQuery,
    ) -> Result<Option<Vec<CommandRecord>>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                let exists = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .filter(devices::id.eq(&device_id))
                    .select(devices::id)
                    .first::<String>(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                    .is_some();
                if !exists {
                    return Ok(None);
                }
                let mut statement = command_history::table
                    .filter(command_history::tenant_id.eq(&tenant_id))
                    .filter(command_history::device_id.eq(&device_id))
                    .into_boxed();
                if let Some(status) = query.status {
                    statement = statement.filter(command_history::status.eq(status));
                }
                let records = statement
                    .order(command_history::created_at.desc())
                    .limit(query.limit)
                    .select(PgCommandRecord::as_select())
                    .load::<PgCommandRecord>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(to_record)
                    .collect();
                Ok(Some(records))
            })
            .await
    }

    async fn apply_response(
        &self,
        identity: &DeviceIdentity,
        correlation_id: String,
        device_status: String,
        payload: Option<String>,
    ) -> Result<Option<String>, PersistenceError> {
        let tenant_id = identity.tenant_id_str().to_string();
        let device_id = identity.device_id().to_string();
        self.executor
            .run(move |connection| {
                let command = command_history::table
                    .filter(command_history::tenant_id.eq(&tenant_id))
                    .filter(command_history::id.eq(&correlation_id))
                    .select(PgCommandRecord::as_select())
                    .first::<PgCommandRecord>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                let Some(command) = command else {
                    return Ok(None);
                };
                if command.device_id != device_id
                    || TERMINAL_STATUSES.contains(&command.status.as_str())
                {
                    return Ok(None);
                }

                let new_status = match device_status.as_str() {
                    "ack" => "delivered",
                    "succeeded" | "failed" => device_status.as_str(),
                    _ => "delivered",
                }
                .to_string();
                diesel::update(
                    command_history::table
                        .filter(command_history::tenant_id.eq(&tenant_id))
                        .filter(command_history::id.eq(&correlation_id))
                        .filter(command_history::device_id.eq(&device_id))
                        .filter(command_history::status.ne_all(TERMINAL_STATUSES)),
                )
                .set((
                    command_history::status.eq(&new_status),
                    command_history::response_payload.eq(payload.as_deref()),
                    command_history::updated_at.eq(chrono::Utc::now().naive_utc()),
                ))
                .execute(connection)
                .map_err(map_diesel_error)
                .map(|rows| (rows == 1).then_some(new_status))
            })
            .await
    }

    async fn timeout_stale(
        &self,
        cutoff: chrono::NaiveDateTime,
        now: chrono::NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        self.executor
            .run(move |connection| {
                diesel::update(
                    command_history::table
                        .filter(command_history::status.eq_any(&["sent", "delivered"]))
                        .filter(command_history::created_at.lt(cutoff)),
                )
                .set((
                    command_history::status.eq("timed_out"),
                    command_history::updated_at.eq(now),
                ))
                .execute(connection)
                .map_err(map_diesel_error)
            })
            .await
    }
}
