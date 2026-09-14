use async_trait::async_trait;
use diesel::Connection;
use diesel::OptionalExtension;
use diesel::prelude::*;

use crate::models::{
    Device, NewTelemetryRecord, TelemetryRecord as PgTelemetryRecord,
    TelemetryRollupHourly as PgTelemetryRollup, UpdateDevice,
};
use extrittio_backend_core::telemetry::TelemetryRepository;
use extrittio_backend_core::telemetry::{
    PartitionMaintenance, TelemetryMaintenanceOutcome, TelemetryQuery, TelemetryRecord,
    TelemetryRollup, TelemetryWrite, TelemetryWriteOutcome,
};

use crate::telemetry_sql as telemetry_repo;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::{DeviceIdentity, TenantId};

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresTelemetryRepository {
    executor: PostgresExecutor,
}
impl PostgresTelemetryRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
#[derive(Debug, thiserror::Error)]
enum TelemetryTransactionError {
    #[error(transparent)]
    Database(#[from] diesel::result::Error),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

use crate::error::map_diesel_error;
use crate::outbox::enqueue_actions_in_transaction as enqueue_pending_actions;

fn to_record(record: PgTelemetryRecord) -> TelemetryRecord {
    TelemetryRecord {
        id: record.id,
        device_id: record.device_id,
        temperature: record.temperature,
        humidity: record.humidity,
        battery_level: record.battery_level,
        custom_json: record.custom_json,
        received_at: record.received_at,
        latitude: record.latitude,
        longitude: record.longitude,
        speed: record.speed,
        altitude: record.altitude,
        heading: record.heading,
    }
}

fn to_rollup(record: PgTelemetryRollup) -> TelemetryRollup {
    TelemetryRollup {
        device_id: record.device_id,
        bucket_start: record.bucket_start,
        sample_count: record.sample_count,
        avg_temperature: record.avg_temperature,
        min_temperature: record.min_temperature,
        max_temperature: record.max_temperature,
        avg_humidity: record.avg_humidity,
        min_humidity: record.min_humidity,
        max_humidity: record.max_humidity,
        avg_battery_level: record.avg_battery_level,
        min_battery_level: record.min_battery_level,
        max_battery_level: record.max_battery_level,
    }
}

fn map_app_error(error: TelemetryTransactionError) -> PersistenceError {
    match error {
        TelemetryTransactionError::Database(error) => map_diesel_error(error),
        TelemetryTransactionError::Persistence(error) => error,
    }
}

#[async_trait]
impl TelemetryRepository for PostgresTelemetryRepository {
    async fn record(
        &self,
        identity: &DeviceIdentity,
        write: TelemetryWrite,
    ) -> Result<TelemetryWriteOutcome, PersistenceError> {
        let tenant_id = identity.tenant_id_str().to_string();
        let device_id = identity.device_id().to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, TelemetryTransactionError, _>(|connection| {
                        use crate::schema::devices;
                        let device = devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::id.eq(&device_id))
                            .select(Device::as_select())
                            .for_update()
                            .first::<Device>(connection)
                            .optional()?;
                        let Some(device) = device else {
                            return Ok(TelemetryWriteOutcome {
                                recorded: false,
                                actions_enqueued: 0,
                            });
                        };
                        if device.device_type_id != write.expected_device_type_id
                            || device.fleet_id != write.expected_fleet_id
                        {
                            return Ok(TelemetryWriteOutcome {
                                recorded: false,
                                actions_enqueued: 0,
                            });
                        }
                        let record = NewTelemetryRecord {
                            tenant_id: tenant_id.clone(),
                            device_id: device_id.clone(),
                            payload: write.payload,
                            temperature: write.temperature,
                            humidity: write.humidity,
                            battery_level: write.battery_level,
                            custom_json: write.custom_json,
                            latitude: write.latitude,
                            longitude: write.longitude,
                            speed: write.speed,
                            altitude: write.altitude,
                            heading: write.heading,
                        };
                        let (telemetry_id, received_at) = telemetry_repo::insert_telemetry(
                            connection,
                            &record,
                            write.received_at,
                        )?;
                        telemetry_repo::upsert_latest_state(
                            connection,
                            &record,
                            telemetry_id,
                            received_at,
                        )?;
                        diesel::update(
                            devices::table
                                .filter(devices::tenant_id.eq(&tenant_id))
                                .filter(devices::id.eq(&device_id)),
                        )
                        .set(UpdateDevice {
                            last_seen: Some(write.observed_at),
                            updated_at: Some(write.observed_at),
                            ..Default::default()
                        })
                        .execute(connection)?;
                        if write.latitude.is_some() && write.longitude.is_some() {
                            diesel::update(
                                devices::table
                                    .filter(devices::tenant_id.eq(&tenant_id))
                                    .filter(devices::id.eq(&device_id)),
                            )
                            .set((
                                devices::latest_latitude.eq(write.latitude),
                                devices::latest_longitude.eq(write.longitude),
                            ))
                            .execute(connection)?;
                        }
                        let actions = crate::rule_runtime::evaluate_rules_in_transaction(
                            connection,
                            &tenant_id,
                            &device_id,
                            Some(&write.rule_evaluation),
                        )?;
                        let actions_enqueued = enqueue_pending_actions(connection, &actions)?;
                        Ok(TelemetryWriteOutcome {
                            recorded: true,
                            actions_enqueued,
                        })
                    })
                    .map_err(map_app_error)
            })
            .await
    }

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRecord>>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                if crate::schema::devices::table
                    .filter(crate::schema::devices::tenant_id.eq(&tenant_id))
                    .filter(crate::schema::devices::id.eq(&device_id))
                    .select(crate::schema::devices::id)
                    .first::<String>(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                    .is_none()
                {
                    return Ok(None);
                }
                telemetry_repo::list_telemetry(
                    connection,
                    &tenant_id,
                    &device_id,
                    query.since,
                    query.before,
                    query.limit,
                )
                .map(|records| Some(records.into_iter().map(to_record).collect()))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn latest(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                telemetry_repo::latest_telemetry(connection, &tenant_id, &device_id)
                    .map(|record| record.map(to_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn list_hourly(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRollup>>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                if crate::schema::devices::table
                    .filter(crate::schema::devices::tenant_id.eq(&tenant_id))
                    .filter(crate::schema::devices::id.eq(&device_id))
                    .select(crate::schema::devices::id)
                    .first::<String>(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                    .is_none()
                {
                    return Ok(None);
                }
                telemetry_repo::list_hourly_rollups(
                    connection,
                    &tenant_id,
                    &device_id,
                    query.since,
                    query.before,
                    query.limit,
                )
                .map(|records| Some(records.into_iter().map(to_rollup).collect()))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn latest_location(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                telemetry_repo::get_latest_location(connection, &tenant_id, &device_id)
                    .map(|record| record.map(to_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn maintain(
        &self,
        rollup_since: chrono::NaiveDateTime,
        rollup_before: chrono::NaiveDateTime,
        retention_cutoff: chrono::NaiveDateTime,
    ) -> Result<TelemetryMaintenanceOutcome, PersistenceError> {
        self.executor
            .run(move |connection| {
                // Partition helpers use transactional DDL and contain no commits.
                // Rollback the whole maintenance pass if any stage fails.
                connection
                    .transaction::<_, TelemetryTransactionError, _>(|connection| {
                        let boundary = telemetry_repo::lock_maintenance_boundary(connection)?;
                        let rollup_since =
                            extrittio_backend_core::telemetry::rollup_recompute_start(
                                rollup_since,
                                boundary,
                            )
                            .ok_or_else(|| {
                                PersistenceError::Internal(
                                    "Telemetry pruning boundary is outside the supported range"
                                        .into(),
                                )
                            })?;
                        let rollups_upserted = telemetry_repo::upsert_hourly_rollups(
                            connection,
                            rollup_since,
                            rollup_before,
                        )?;
                        let partitions =
                            telemetry_repo::maintain_partitions(connection, 3, retention_cutoff)?;
                        let rows_deleted =
                            telemetry_repo::delete_older_than(connection, retention_cutoff)?;
                        telemetry_repo::advance_maintenance_boundary(connection, retention_cutoff)?;
                        Ok(TelemetryMaintenanceOutcome {
                            rollups_upserted,
                            rows_deleted,
                            partitions: PartitionMaintenance {
                                created_count: partitions.created_count,
                                dropped_count: partitions.dropped_count,
                            },
                        })
                    })
                    .map_err(map_app_error)
            })
            .await
    }
}
