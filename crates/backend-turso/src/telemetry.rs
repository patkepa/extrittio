use async_trait::async_trait;
use chrono::NaiveDateTime;
use turso::{Connection, Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::telemetry::TelemetryRepository;
use extrittio_backend_core::telemetry::{
    PartitionMaintenance, TelemetryMaintenanceOutcome, TelemetryQuery, TelemetryRecord,
    TelemetryRollup, TelemetryWrite, TelemetryWriteOutcome,
};
use extrittio_backend_core::{DeviceIdentity, TenantId};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoTelemetryRepository {
    handles: TursoConnectionHandles,
}
impl TursoTelemetryRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}
use crate::outbox::enqueue_actions_in_transaction as enqueue;

fn f32_at(record: &Row, index: usize) -> Result<Option<f32>, PersistenceError> {
    Ok(record
        .get::<Option<f64>>(index)
        .map_err(row::legacy_error)?
        .map(|value| value as f32))
}

fn decode(record: &Row) -> Result<TelemetryRecord, PersistenceError> {
    let custom_json = record
        .get::<Option<String>>(5)
        .map_err(row::legacy_error)?
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
    Ok(TelemetryRecord {
        id: record.get(0).map_err(row::legacy_error)?,
        device_id: record.get(1).map_err(row::legacy_error)?,
        temperature: f32_at(record, 2)?,
        humidity: f32_at(record, 3)?,
        battery_level: f32_at(record, 4)?,
        custom_json,
        received_at: row::datetime(record.get(6).map_err(row::legacy_error)?)?.naive_utc(),
        latitude: record.get(7).map_err(row::legacy_error)?,
        longitude: record.get(8).map_err(row::legacy_error)?,
        speed: f32_at(record, 9)?,
        altitude: f32_at(record, 10)?,
        heading: f32_at(record, 11)?,
    })
}

fn decode_rollup(record: &Row) -> Result<TelemetryRollup, PersistenceError> {
    Ok(TelemetryRollup {
        device_id: record.get(0).map_err(row::legacy_error)?,
        bucket_start: row::datetime(record.get(1).map_err(row::legacy_error)?)?.naive_utc(),
        sample_count: record.get(2).map_err(row::legacy_error)?,
        avg_temperature: f32_at(record, 3)?,
        min_temperature: f32_at(record, 4)?,
        max_temperature: f32_at(record, 5)?,
        avg_humidity: f32_at(record, 6)?,
        min_humidity: f32_at(record, 7)?,
        max_humidity: f32_at(record, 8)?,
        avg_battery_level: f32_at(record, 9)?,
        min_battery_level: f32_at(record, 10)?,
        max_battery_level: f32_at(record, 11)?,
    })
}

async fn exists(
    connection: &Connection,
    tenant: &TenantId,
    device_id: &str,
) -> Result<bool, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT EXISTS(SELECT 1 FROM devices WHERE tenant_id = ?1 AND id = ?2)",
            params![tenant.as_str(), device_id],
        )
        .await
        .map_err(row::legacy_error)?;
    Ok(rows
        .next()
        .await
        .map_err(row::legacy_error)?
        .ok_or(PersistenceError::NotFound)?
        .get::<i64>(0)
        .map_err(row::legacy_error)?
        != 0)
}

const SELECT_RECORD: &str = "SELECT id, device_id, temperature, humidity, battery_level, custom_json, received_at, latitude, longitude, speed, altitude, heading FROM telemetry";

#[async_trait]
impl TelemetryRepository for TursoTelemetryRepository {
    async fn record(
        &self,
        identity: &DeviceIdentity,
        write: TelemetryWrite,
    ) -> Result<TelemetryWriteOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let mut device_rows = transaction
            .query(
                "SELECT device_type_id, fleet_id FROM devices WHERE tenant_id = ?1 AND id = ?2",
                params![identity.tenant_id_str(), identity.device_id()],
            )
            .await
            .map_err(row::legacy_error)?;
        let Some(device) = device_rows.next().await.map_err(row::legacy_error)? else {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(TelemetryWriteOutcome {
                recorded: false,
                actions_enqueued: 0,
            });
        };
        let actual_type = row::i32(
            device.get(0).map_err(row::legacy_error)?,
            "devices.device_type_id",
        )?;
        let actual_fleet = device
            .get::<Option<i64>>(1)
            .map_err(row::legacy_error)?
            .map(|value| row::i32(value, "devices.fleet_id"))
            .transpose()?;
        drop(device_rows);
        if actual_type != write.expected_device_type_id || actual_fleet != write.expected_fleet_id {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(TelemetryWriteOutcome {
                recorded: false,
                actions_enqueued: 0,
            });
        }
        let observed_at = write.observed_at.and_utc().timestamp_micros();
        let custom_json = write
            .custom_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        transaction.execute(
            "INSERT INTO telemetry (tenant_id, device_id, payload, temperature, humidity, battery_level, custom_json, latitude, longitude, speed, altitude, heading, received_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![identity.tenant_id_str(), identity.device_id(), write.payload,
                write.temperature.map(f64::from), write.humidity.map(f64::from), write.battery_level.map(f64::from), custom_json,
                write.latitude, write.longitude, write.speed.map(f64::from), write.altitude.map(f64::from), write.heading.map(f64::from), write.received_at.and_utc().timestamp_micros()],
        ).await.map_err(row::legacy_error)?;
        transaction.execute(
            "UPDATE devices SET last_seen = ?3, updated_at = ?3,
                    latest_latitude = CASE WHEN ?4 IS NOT NULL AND ?5 IS NOT NULL THEN ?4 ELSE latest_latitude END,
                    latest_longitude = CASE WHEN ?4 IS NOT NULL AND ?5 IS NOT NULL THEN ?5 ELSE latest_longitude END
             WHERE tenant_id = ?1 AND id = ?2",
            params![identity.tenant_id_str(), identity.device_id(), observed_at, write.latitude, write.longitude],
        ).await.map_err(row::legacy_error)?;
        let actions = crate::rule_runtime::evaluate_rules_in_transaction(
            &transaction,
            identity.tenant_id_str(),
            identity.device_id(),
            Some(&write.rule_evaluation),
        )
        .await?;
        let actions_enqueued = enqueue(&transaction, &actions).await?;
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(TelemetryWriteOutcome {
            recorded: true,
            actions_enqueued,
        })
    }

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRecord>>, PersistenceError> {
        let connection = self.connect()?;
        if !exists(&connection, tenant, device_id).await? {
            return Ok(None);
        }
        let mut rows = connection.query(&format!("{SELECT_RECORD} WHERE tenant_id = ?1 AND device_id = ?2 AND (?3 IS NULL OR received_at >= ?3) AND (?4 IS NULL OR received_at < ?4) ORDER BY received_at DESC, id DESC LIMIT ?5"), params![tenant.as_str(), device_id, query.since.map(|v| v.and_utc().timestamp_micros()), query.before.map(|v| v.and_utc().timestamp_micros()), query.limit]).await.map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            records.push(decode(&record)?);
        }
        Ok(Some(records))
    }

    async fn latest(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection.query(&format!("{SELECT_RECORD} WHERE tenant_id = ?1 AND device_id = ?2 ORDER BY received_at DESC, id DESC LIMIT 1"), params![tenant.as_str(), device_id]).await.map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn list_hourly(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRollup>>, PersistenceError> {
        let connection = self.connect()?;
        if !exists(&connection, tenant, device_id).await? {
            return Ok(None);
        }
        let mut rows = connection.query("SELECT device_id, bucket_start, sample_count, avg_temperature, min_temperature, max_temperature, avg_humidity, min_humidity, max_humidity, avg_battery_level, min_battery_level, max_battery_level FROM telemetry_rollups_hourly WHERE tenant_id = ?1 AND device_id = ?2 AND (?3 IS NULL OR bucket_start >= ?3) AND (?4 IS NULL OR bucket_start < ?4) ORDER BY bucket_start DESC LIMIT ?5", params![tenant.as_str(), device_id, query.since.map(|v| v.and_utc().timestamp_micros()), query.before.map(|v| v.and_utc().timestamp_micros()), query.limit]).await.map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            records.push(decode_rollup(&record)?);
        }
        Ok(Some(records))
    }

    async fn latest_location(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection.query(&format!("{SELECT_RECORD} WHERE tenant_id = ?1 AND device_id = ?2 AND latitude IS NOT NULL AND longitude IS NOT NULL ORDER BY received_at DESC, id DESC LIMIT 1"), params![tenant.as_str(), device_id]).await.map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn maintain(
        &self,
        rollup_since: NaiveDateTime,
        rollup_before: NaiveDateTime,
        retention_cutoff: NaiveDateTime,
    ) -> Result<TelemetryMaintenanceOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let mut boundary_rows = transaction
            .query(
                "SELECT pruned_before FROM telemetry_maintenance_state WHERE singleton = 1",
                (),
            )
            .await
            .map_err(row::legacy_error)?;
        let boundary_row = boundary_rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or_else(|| {
                PersistenceError::Internal("Telemetry maintenance state is missing".into())
            })?;
        let boundary_us = boundary_row
            .get::<Option<i64>>(0)
            .map_err(row::legacy_error)?;
        let boundary = boundary_us
            .map(|value| {
                chrono::DateTime::from_timestamp_micros(value)
                    .map(|time| time.naive_utc())
                    .ok_or_else(|| {
                        PersistenceError::Internal("Invalid telemetry pruning boundary".into())
                    })
            })
            .transpose()?;
        drop(boundary_rows);
        let rollup_since =
            extrittio_backend_core::telemetry::rollup_recompute_start(rollup_since, boundary)
                .ok_or_else(|| {
                    PersistenceError::Internal(
                        "Telemetry pruning boundary is outside the supported range".into(),
                    )
                })?;
        let now = chrono::Utc::now().timestamp_micros();
        let rollups_upserted = transaction.execute(
            "INSERT INTO telemetry_rollups_hourly (tenant_id, device_id, bucket_start, sample_count, avg_temperature, min_temperature, max_temperature, avg_humidity, min_humidity, max_humidity, avg_battery_level, min_battery_level, max_battery_level, created_at, updated_at)
             SELECT tenant_id, device_id, received_at - ((received_at % 3600000000 + 3600000000) % 3600000000), count(*), avg(temperature), min(temperature), max(temperature), avg(humidity), min(humidity), max(humidity), avg(battery_level), min(battery_level), max(battery_level), ?3, ?3 FROM telemetry WHERE received_at >= ?1 AND received_at < ?2 GROUP BY tenant_id, device_id, received_at - ((received_at % 3600000000 + 3600000000) % 3600000000)
             ON CONFLICT (tenant_id, device_id, bucket_start) DO UPDATE SET sample_count = excluded.sample_count, avg_temperature = excluded.avg_temperature, min_temperature = excluded.min_temperature, max_temperature = excluded.max_temperature, avg_humidity = excluded.avg_humidity, min_humidity = excluded.min_humidity, max_humidity = excluded.max_humidity, avg_battery_level = excluded.avg_battery_level, min_battery_level = excluded.min_battery_level, max_battery_level = excluded.max_battery_level, updated_at = excluded.updated_at",
            params![rollup_since.and_utc().timestamp_micros(), rollup_before.and_utc().timestamp_micros(), now],
        ).await.map_err(row::legacy_error)? as usize;
        let rows_deleted = transaction
            .execute(
                "DELETE FROM telemetry WHERE received_at < ?1",
                params![retention_cutoff.and_utc().timestamp_micros()],
            )
            .await
            .map_err(row::legacy_error)? as usize;
        transaction.execute(
            "UPDATE telemetry_maintenance_state SET pruned_before = CASE WHEN pruned_before IS NULL OR pruned_before < ?1 THEN ?1 ELSE pruned_before END WHERE singleton = 1",
            params![retention_cutoff.and_utc().timestamp_micros()],
        ).await.map_err(row::legacy_error)?;
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(TelemetryMaintenanceOutcome {
            rollups_upserted,
            rows_deleted,
            partitions: PartitionMaintenance {
                created_count: 0,
                dropped_count: 0,
            },
        })
    }
}
