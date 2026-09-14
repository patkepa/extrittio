use async_trait::async_trait;
use diesel::prelude::*;

use crate::models::{CommandRecord as PgCommandRecord, NewCommandRecord as PgNewCommandRecord};
use crate::schema::{command_history, devices};
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::commands::CommandRepository;
use extrittio_backend_core::commands::{CommandQuery, CommandRecord, NewCommandRecord};

use crate::{PostgresExecutor, PostgresPool};
use extrittio_backend_core::commands::ACTIVE_STATUSES;
#[derive(Clone)]
pub struct PostgresCommandRepository {
    executor: PostgresExecutor,
}
impl PostgresCommandRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
use crate::error::map_diesel_error;

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
impl CommandRepository for PostgresCommandRepository {
    async fn find(
        &self,
        tenant: &TenantId,
        id: &str,
    ) -> Result<Option<CommandRecord>, PersistenceError> {
        let tenant = tenant.as_str().to_owned();
        let id = id.to_owned();
        self.executor
            .run(move |connection| {
                command_history::table
                    .filter(command_history::tenant_id.eq(tenant))
                    .filter(command_history::id.eq(id))
                    .select(PgCommandRecord::as_select())
                    .first::<PgCommandRecord>(connection)
                    .optional()
                    .map(|row| row.map(to_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

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
                    .order((
                        command_history::created_at.desc(),
                        command_history::id.desc(),
                    ))
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
        tenant: &TenantId,
        device_id: &str,
        correlation_id: String,
        status: extrittio_backend_core::commands::CommandResponseStatus,
        payload: Option<String>,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<Option<String>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        let new_status = status.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::update(
                    command_history::table
                        .filter(command_history::tenant_id.eq(&tenant_id))
                        .filter(command_history::id.eq(&correlation_id))
                        .filter(command_history::device_id.eq(&device_id))
                        .filter(command_history::status.eq_any(ACTIVE_STATUSES)),
                )
                .set((
                    command_history::status.eq(&new_status),
                    command_history::response_payload.eq(payload.as_deref()),
                    command_history::updated_at.eq(updated_at),
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
                        .filter(command_history::status.eq_any(ACTIVE_STATUSES))
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
