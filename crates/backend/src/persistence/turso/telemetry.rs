use async_trait::async_trait;
use chrono::NaiveDateTime;
use turso::{Connection, Row, params};

use crate::domains::telemetry::port::TelemetryRepository;
use crate::domains::telemetry::types::{
    PartitionMaintenance, TelemetryMaintenanceOutcome, TelemetryQuery, TelemetryRecord,
    TelemetryRollup, TelemetryWrite, TelemetryWriteOutcome,
};
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::{TursoAdapter, devices::enqueue, row};

fn f32_at(record: &Row, index: usize) -> Result<Option<f32>, PersistenceError> {
    Ok(record
        .get::<Option<f64>>(index)
        .map_err(row::error)?
        .map(|value| value as f32))
}

fn decode(record: &Row) -> Result<TelemetryRecord, PersistenceError> {
    let custom_json = record
        .get::<Option<String>>(5)
        .map_err(row::error)?
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
    Ok(TelemetryRecord {
        id: record.get(0).map_err(row::error)?,
        device_id: record.get(1).map_err(row::error)?,
        temperature: f32_at(record, 2)?,
        humidity: f32_at(record, 3)?,
        battery_level: f32_at(record, 4)?,
        custom_json,
        received_at: row::datetime(record.get(6).map_err(row::error)?)?.naive_utc(),
        latitude: record.get(7).map_err(row::error)?,
        longitude: record.get(8).map_err(row::error)?,
        speed: f32_at(record, 9)?,
        altitude: f32_at(record, 10)?,
        heading: f32_at(record, 11)?,
    })
}

fn decode_rollup(record: &Row) -> Result<TelemetryRollup, PersistenceError> {
    Ok(TelemetryRollup {
        device_id: record.get(0).map_err(row::error)?,
        bucket_start: row::datetime(record.get(1).map_err(row::error)?)?.naive_utc(),
        sample_count: record.get(2).map_err(row::error)?,
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
        .map_err(row::error)?;
    Ok(rows
        .next()
        .await
        .map_err(row::error)?
        .ok_or(PersistenceError::NotFound)?
        .get::<i64>(0)
        .map_err(row::error)?
        != 0)
}

const SELECT_RECORD: &str = "SELECT id, device_id, temperature, humidity, battery_level, custom_json, received_at, latitude, longitude, speed, altitude, heading FROM telemetry";

#[async_trait]
impl TelemetryRepository for TursoAdapter {
    async fn record(
        &self,
        identity: &DeviceIdentity,
        write: TelemetryWrite,
    ) -> Result<TelemetryWriteOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let mut device_rows = transaction
            .query(
                "SELECT device_type_id, fleet_id FROM devices WHERE tenant_id = ?1 AND id = ?2",
                params![identity.tenant_id_str(), identity.device_id()],
            )
            .await
            .map_err(row::error)?;
        let Some(device) = device_rows.next().await.map_err(row::error)? else {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(TelemetryWriteOutcome {
                recorded: false,
                actions_enqueued: 0,
            });
        };
        let actual_type = row::i32(device.get(0).map_err(row::error)?, "devices.device_type_id")?;
        let actual_fleet = device
            .get::<Option<i64>>(1)
            .map_err(row::error)?
            .map(|value| row::i32(value, "devices.fleet_id"))
            .transpose()?;
        drop(device_rows);
        if actual_type != write.expected_device_type_id || actual_fleet != write.expected_fleet_id {
            transaction.rollback().await.map_err(row::error)?;
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
                write.latitude, write.longitude, write.speed.map(f64::from), write.altitude.map(f64::from), write.heading.map(f64::from), observed_at],
        ).await.map_err(row::error)?;
        if let Some(hosts) = write.observed_network_hosts {
            transaction.execute("UPDATE network_observed_hosts SET status = 'inactive', updated_at = ?3 WHERE tenant_id = ?1 AND analyzer_device_id = ?2 AND status <> 'inactive'", params![identity.tenant_id_str(), identity.device_id(), observed_at]).await.map_err(row::error)?;
            for host in hosts {
                transaction.execute(
                    "INSERT INTO network_observed_hosts (tenant_id, analyzer_device_id, host_key, label, address, device_type, source, status, first_seen_at, last_seen_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?8, ?8, ?8)
                     ON CONFLICT (tenant_id, analyzer_device_id, host_key) DO UPDATE SET label = excluded.label, address = excluded.address, device_type = excluded.device_type, source = excluded.source, status = 'active', last_seen_at = excluded.last_seen_at, updated_at = excluded.updated_at",
                    params![identity.tenant_id_str(), identity.device_id(), host.host_key, host.label, host.address, host.device_type, host.source, observed_at],
                ).await.map_err(row::error)?;
            }
            transaction
                .execute(
                    "DELETE FROM network_observed_hosts WHERE last_seen_at < ?1",
                    params![
                        (write.observed_at - chrono::Duration::days(30))
                            .and_utc()
                            .timestamp_micros()
                    ],
                )
                .await
                .map_err(row::error)?;
        }
        let connections = write
            .declared_connections
            .map(|value| serde_json::to_string(&value))
            .transpose()
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        transaction.execute(
            "UPDATE devices SET last_seen = ?3, updated_at = ?3, declared_connections = COALESCE(?4, declared_connections),
                    latest_latitude = CASE WHEN ?5 IS NOT NULL AND ?6 IS NOT NULL THEN ?5 ELSE latest_latitude END,
                    latest_longitude = CASE WHEN ?5 IS NOT NULL AND ?6 IS NOT NULL THEN ?6 ELSE latest_longitude END
             WHERE tenant_id = ?1 AND id = ?2",
            params![identity.tenant_id_str(), identity.device_id(), observed_at, connections, write.latitude, write.longitude],
        ).await.map_err(row::error)?;
        let actions_enqueued = enqueue(&transaction, &write.pending_actions).await?;
        transaction.commit().await.map_err(row::error)?;
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
        let connection = self.database.connect()?;
        if !exists(&connection, tenant, device_id).await? {
            return Ok(None);
        }
        let mut rows = connection.query(&format!("{SELECT_RECORD} WHERE tenant_id = ?1 AND device_id = ?2 AND (?3 IS NULL OR received_at >= ?3) AND (?4 IS NULL OR received_at < ?4) ORDER BY received_at DESC, id DESC LIMIT ?5"), params![tenant.as_str(), device_id, query.since.map(|v| v.and_utc().timestamp_micros()), query.before.map(|v| v.and_utc().timestamp_micros()), query.limit]).await.map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            records.push(decode(&record)?);
        }
        Ok(Some(records))
    }

    async fn latest(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection.query(&format!("{SELECT_RECORD} WHERE tenant_id = ?1 AND device_id = ?2 ORDER BY received_at DESC, id DESC LIMIT 1"), params![tenant.as_str(), device_id]).await.map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn list_hourly(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRollup>>, PersistenceError> {
        let connection = self.database.connect()?;
        if !exists(&connection, tenant, device_id).await? {
            return Ok(None);
        }
        let mut rows = connection.query("SELECT device_id, bucket_start, sample_count, avg_temperature, min_temperature, max_temperature, avg_humidity, min_humidity, max_humidity, avg_battery_level, min_battery_level, max_battery_level FROM telemetry_rollups_hourly WHERE tenant_id = ?1 AND device_id = ?2 AND (?3 IS NULL OR bucket_start >= ?3) AND (?4 IS NULL OR bucket_start < ?4) ORDER BY bucket_start DESC LIMIT ?5", params![tenant.as_str(), device_id, query.since.map(|v| v.and_utc().timestamp_micros()), query.before.map(|v| v.and_utc().timestamp_micros()), query.limit]).await.map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            records.push(decode_rollup(&record)?);
        }
        Ok(Some(records))
    }

    async fn latest_location(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection.query(&format!("{SELECT_RECORD} WHERE tenant_id = ?1 AND device_id = ?2 AND latitude IS NOT NULL AND longitude IS NOT NULL ORDER BY received_at DESC, id DESC LIMIT 1"), params![tenant.as_str(), device_id]).await.map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn maintain(
        &self,
        rollup_since: NaiveDateTime,
        rollup_before: NaiveDateTime,
        retention_cutoff: NaiveDateTime,
    ) -> Result<TelemetryMaintenanceOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let now = chrono::Utc::now().timestamp_micros();
        let rollups_upserted = transaction.execute(
            "INSERT INTO telemetry_rollups_hourly (tenant_id, device_id, bucket_start, sample_count, avg_temperature, min_temperature, max_temperature, avg_humidity, min_humidity, max_humidity, avg_battery_level, min_battery_level, max_battery_level, created_at, updated_at)
             SELECT tenant_id, device_id, received_at - (received_at % 3600000000), count(*), avg(temperature), min(temperature), max(temperature), avg(humidity), min(humidity), max(humidity), avg(battery_level), min(battery_level), max(battery_level), ?3, ?3 FROM telemetry WHERE received_at >= ?1 AND received_at < ?2 GROUP BY tenant_id, device_id, received_at - (received_at % 3600000000)
             ON CONFLICT (tenant_id, device_id, bucket_start) DO UPDATE SET sample_count = excluded.sample_count, avg_temperature = excluded.avg_temperature, min_temperature = excluded.min_temperature, max_temperature = excluded.max_temperature, avg_humidity = excluded.avg_humidity, min_humidity = excluded.min_humidity, max_humidity = excluded.max_humidity, avg_battery_level = excluded.avg_battery_level, min_battery_level = excluded.min_battery_level, max_battery_level = excluded.max_battery_level, updated_at = excluded.updated_at",
            params![rollup_since.and_utc().timestamp_micros(), rollup_before.and_utc().timestamp_micros(), now],
        ).await.map_err(row::error)? as usize;
        let rows_deleted = transaction
            .execute(
                "DELETE FROM telemetry WHERE received_at < ?1",
                params![retention_cutoff.and_utc().timestamp_micros()],
            )
            .await
            .map_err(row::error)? as usize;
        transaction.commit().await.map_err(row::error)?;
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
