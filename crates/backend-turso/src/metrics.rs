use async_trait::async_trait;
use chrono::NaiveDateTime;
use turso::{Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::metrics::MetricsRepository;
use extrittio_backend_core::metrics::{
    AppMetricRecord, MetricsHistory, MetricsSnapshot, NewAppMetricRecord, NewSystemMetricRecord,
    SystemMetricRecord,
};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoMetricsRepository {
    handles: TursoConnectionHandles,
}
impl TursoMetricsRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
}

fn f32_value(record: &Row, index: usize) -> Result<f32, PersistenceError> {
    Ok(record.get::<f64>(index).map_err(row::legacy_error)? as f32)
}
fn i32_value(record: &Row, index: usize, name: &str) -> Result<i32, PersistenceError> {
    row::i32(record.get(index).map_err(row::legacy_error)?, name)
}
fn system(record: &Row) -> Result<SystemMetricRecord, PersistenceError> {
    Ok(SystemMetricRecord {
        cpu_usage_percent: f32_value(record, 0)?,
        memory_used_bytes: record.get(1).map_err(row::legacy_error)?,
        memory_total_bytes: record.get(2).map_err(row::legacy_error)?,
        disk_used_bytes: record.get(3).map_err(row::legacy_error)?,
        disk_total_bytes: record.get(4).map_err(row::legacy_error)?,
        network_rx_bytes_delta: record.get(5).map_err(row::legacy_error)?,
        network_tx_bytes_delta: record.get(6).map_err(row::legacy_error)?,
        load_avg_1m: f32_value(record, 7)?,
        load_avg_5m: f32_value(record, 8)?,
        load_avg_15m: f32_value(record, 9)?,
        recorded_at: row::datetime(record.get(10).map_err(row::legacy_error)?)?.naive_utc(),
    })
}
fn app(record: &Row) -> Result<AppMetricRecord, PersistenceError> {
    Ok(AppMetricRecord {
        request_count: i32_value(record, 0, "app_metrics.request_count")?,
        error_count: i32_value(record, 1, "app_metrics.error_count")?,
        avg_latency_ms: f32_value(record, 2)?,
        p95_latency_ms: f32_value(record, 3)?,
        db_pool_active: i32_value(record, 4, "app_metrics.db_pool_active")?,
        db_pool_idle: i32_value(record, 5, "app_metrics.db_pool_idle")?,
        zenoh_messages_in: i32_value(record, 6, "app_metrics.zenoh_messages_in")?,
        zenoh_messages_out: i32_value(record, 7, "app_metrics.zenoh_messages_out")?,
        recorded_at: row::datetime(record.get(8).map_err(row::legacy_error)?)?.naive_utc(),
    })
}

const SYSTEM_COLUMNS: &str = "cpu_usage_percent, memory_used_bytes, memory_total_bytes, disk_used_bytes, disk_total_bytes, network_rx_bytes_delta, network_tx_bytes_delta, load_avg_1m, load_avg_5m, load_avg_15m, recorded_at";
const APP_COLUMNS: &str = "request_count, error_count, avg_latency_ms, p95_latency_ms, db_pool_active, db_pool_idle, zenoh_messages_in, zenoh_messages_out, recorded_at";

#[async_trait]
impl MetricsRepository for TursoMetricsRepository {
    async fn insert_system(&self, record: NewSystemMetricRecord) -> Result<(), PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer.execute("INSERT INTO server_metrics (cpu_usage_percent, memory_used_bytes, memory_total_bytes, disk_used_bytes, disk_total_bytes, network_rx_bytes_delta, network_tx_bytes_delta, load_avg_1m, load_avg_5m, load_avg_15m, recorded_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", params![f64::from(record.cpu_usage_percent), record.memory_used_bytes, record.memory_total_bytes, record.disk_used_bytes, record.disk_total_bytes, record.network_rx_bytes_delta, record.network_tx_bytes_delta, f64::from(record.load_avg_1m), f64::from(record.load_avg_5m), f64::from(record.load_avg_15m), chrono::Utc::now().timestamp_micros()]).await.map(|_| ()).map_err(row::legacy_error)
    }
    async fn insert_app(&self, record: NewAppMetricRecord) -> Result<(), PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer.execute("INSERT INTO app_metrics (request_count,error_count,avg_latency_ms,p95_latency_ms,db_pool_active,db_pool_idle,zenoh_messages_in,zenoh_messages_out,recorded_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![i64::from(record.request_count), i64::from(record.error_count), f64::from(record.avg_latency_ms), f64::from(record.p95_latency_ms), i64::from(record.db_pool_active), i64::from(record.db_pool_idle), i64::from(record.zenoh_messages_in), i64::from(record.zenoh_messages_out), chrono::Utc::now().timestamp_micros()]).await.map(|_| ()).map_err(row::legacy_error)
    }
    async fn current(&self) -> Result<MetricsSnapshot, PersistenceError> {
        let connection = self
            .handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let mut sr = connection.query(&format!("SELECT {SYSTEM_COLUMNS} FROM server_metrics ORDER BY recorded_at DESC, id DESC LIMIT 1"), ()).await.map_err(row::legacy_error)?;
        let system_metric = sr
            .next()
            .await
            .map_err(row::legacy_error)?
            .map(|r| system(&r))
            .transpose()?;
        let mut ar = connection.query(&format!("SELECT {APP_COLUMNS} FROM app_metrics ORDER BY recorded_at DESC, id DESC LIMIT 1"), ()).await.map_err(row::legacy_error)?;
        let app_metric = ar
            .next()
            .await
            .map_err(row::legacy_error)?
            .map(|r| app(&r))
            .transpose()?;
        Ok(MetricsSnapshot {
            system: system_metric,
            app: app_metric,
        })
    }
    async fn history(
        &self,
        since: NaiveDateTime,
        resolution_secs: i64,
    ) -> Result<MetricsHistory, PersistenceError> {
        let connection = self
            .handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let since = since.and_utc().timestamp_micros();
        let bucket = resolution_secs
            .max(1)
            .checked_mul(1_000_000)
            .ok_or_else(|| {
                PersistenceError::Internal("metrics resolution exceeds microsecond range".into())
            })?;
        let system_sql = if resolution_secs > 10 {
            "SELECT avg(cpu_usage_percent), (sum(memory_used_bytes) / count(*)), (sum(memory_total_bytes) / count(*)), (sum(disk_used_bytes) / count(*)), (sum(disk_total_bytes) / count(*)), sum(network_rx_bytes_delta), sum(network_tx_bytes_delta), avg(load_avg_1m), avg(load_avg_5m), avg(load_avg_15m), (recorded_at - CASE WHEN recorded_at % ?2 < 0 THEN recorded_at % ?2 + ?2 ELSE recorded_at % ?2 END) FROM server_metrics WHERE recorded_at>=?1 GROUP BY (recorded_at - CASE WHEN recorded_at % ?2 < 0 THEN recorded_at % ?2 + ?2 ELSE recorded_at % ?2 END) ORDER BY 11".to_string()
        } else {
            format!(
                "SELECT {SYSTEM_COLUMNS} FROM server_metrics WHERE recorded_at>=?1 ORDER BY recorded_at ASC, id ASC LIMIT 10000"
            )
        };
        let mut sr = if resolution_secs > 10 {
            connection.query(&system_sql, params![since, bucket]).await
        } else {
            connection.query(&system_sql, params![since]).await
        }
        .map_err(row::legacy_error)?;
        let mut systems = Vec::new();
        while let Some(r) = sr.next().await.map_err(row::legacy_error)? {
            systems.push(system(&r)?);
        }
        let app_sql = if resolution_secs > 10 {
            "SELECT sum(request_count),sum(error_count),avg(avg_latency_ms),max(p95_latency_ms),(sum(db_pool_active) / count(*)),(sum(db_pool_idle) / count(*)),sum(zenoh_messages_in),sum(zenoh_messages_out),(recorded_at - CASE WHEN recorded_at % ?2 < 0 THEN recorded_at % ?2 + ?2 ELSE recorded_at % ?2 END) FROM app_metrics WHERE recorded_at>=?1 GROUP BY (recorded_at - CASE WHEN recorded_at % ?2 < 0 THEN recorded_at % ?2 + ?2 ELSE recorded_at % ?2 END) ORDER BY 9".to_string()
        } else {
            format!(
                "SELECT {APP_COLUMNS} FROM app_metrics WHERE recorded_at>=?1 ORDER BY recorded_at ASC, id ASC LIMIT 10000"
            )
        };
        let mut ar = if resolution_secs > 10 {
            connection.query(&app_sql, params![since, bucket]).await
        } else {
            connection.query(&app_sql, params![since]).await
        }
        .map_err(row::legacy_error)?;
        let mut apps = Vec::new();
        while let Some(r) = ar.next().await.map_err(row::legacy_error)? {
            apps.push(app(&r)?);
        }
        Ok(MetricsHistory {
            system: systems,
            app: apps,
        })
    }
    async fn delete_before(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<(usize, usize), PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let tx = writer.transaction().await.map_err(row::legacy_error)?;
        let cutoff = cutoff.and_utc().timestamp_micros();
        let system = tx
            .execute(
                "DELETE FROM server_metrics WHERE recorded_at<?1",
                params![cutoff],
            )
            .await
            .map_err(row::legacy_error)? as usize;
        let app = tx
            .execute(
                "DELETE FROM app_metrics WHERE recorded_at<?1",
                params![cutoff],
            )
            .await
            .map_err(row::legacy_error)? as usize;
        tx.commit().await.map_err(row::legacy_error)?;
        Ok((system, app))
    }
}
