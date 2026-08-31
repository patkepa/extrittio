use async_trait::async_trait;
use chrono::NaiveDateTime;
use turso::{Row, params};

use crate::domains::operations::metrics_repository::MetricsRepository;
use crate::domains::operations::metrics_types::{
    AppMetricRecord, MetricsHistory, MetricsSnapshot, NewAppMetricRecord, NewSystemMetricRecord,
    SystemMetricRecord,
};
use crate::persistence::PersistenceError;

use super::{TursoAdapter, row};

fn f32_value(record: &Row, index: usize) -> Result<f32, PersistenceError> {
    Ok(record.get::<f64>(index).map_err(row::error)? as f32)
}
fn i32_value(record: &Row, index: usize, name: &str) -> Result<i32, PersistenceError> {
    row::i32(record.get(index).map_err(row::error)?, name)
}
fn system(record: &Row) -> Result<SystemMetricRecord, PersistenceError> {
    Ok(SystemMetricRecord {
        cpu_usage_percent: f32_value(record, 0)?,
        memory_used_bytes: record.get(1).map_err(row::error)?,
        memory_total_bytes: record.get(2).map_err(row::error)?,
        disk_used_bytes: record.get(3).map_err(row::error)?,
        disk_total_bytes: record.get(4).map_err(row::error)?,
        network_rx_bytes_delta: record.get(5).map_err(row::error)?,
        network_tx_bytes_delta: record.get(6).map_err(row::error)?,
        load_avg_1m: f32_value(record, 7)?,
        load_avg_5m: f32_value(record, 8)?,
        load_avg_15m: f32_value(record, 9)?,
        recorded_at: row::datetime(record.get(10).map_err(row::error)?)?.naive_utc(),
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
        recorded_at: row::datetime(record.get(8).map_err(row::error)?)?.naive_utc(),
    })
}

const SYSTEM_COLUMNS: &str = "cpu_usage_percent, memory_used_bytes, memory_total_bytes, disk_used_bytes, disk_total_bytes, network_rx_bytes_delta, network_tx_bytes_delta, load_avg_1m, load_avg_5m, load_avg_15m, recorded_at";
const APP_COLUMNS: &str = "request_count, error_count, avg_latency_ms, p95_latency_ms, db_pool_active, db_pool_idle, zenoh_messages_in, zenoh_messages_out, recorded_at";

#[async_trait]
impl MetricsRepository for TursoAdapter {
    async fn insert_system(&self, record: NewSystemMetricRecord) -> Result<(), PersistenceError> {
        let writer = self.database.writer().await;
        writer.execute("INSERT INTO server_metrics (cpu_usage_percent, memory_used_bytes, memory_total_bytes, disk_used_bytes, disk_total_bytes, network_rx_bytes_delta, network_tx_bytes_delta, load_avg_1m, load_avg_5m, load_avg_15m, recorded_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", params![f64::from(record.cpu_usage_percent), record.memory_used_bytes, record.memory_total_bytes, record.disk_used_bytes, record.disk_total_bytes, record.network_rx_bytes_delta, record.network_tx_bytes_delta, f64::from(record.load_avg_1m), f64::from(record.load_avg_5m), f64::from(record.load_avg_15m), chrono::Utc::now().timestamp_micros()]).await.map(|_| ()).map_err(row::error)
    }
    async fn insert_app(&self, record: NewAppMetricRecord) -> Result<(), PersistenceError> {
        let writer = self.database.writer().await;
        writer.execute("INSERT INTO app_metrics (request_count,error_count,avg_latency_ms,p95_latency_ms,db_pool_active,db_pool_idle,zenoh_messages_in,zenoh_messages_out,recorded_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![i64::from(record.request_count), i64::from(record.error_count), f64::from(record.avg_latency_ms), f64::from(record.p95_latency_ms), i64::from(record.db_pool_active), i64::from(record.db_pool_idle), i64::from(record.zenoh_messages_in), i64::from(record.zenoh_messages_out), chrono::Utc::now().timestamp_micros()]).await.map(|_| ()).map_err(row::error)
    }
    async fn current(&self) -> Result<MetricsSnapshot, PersistenceError> {
        let connection = self.database.connect()?;
        let mut sr = connection.query(&format!("SELECT {SYSTEM_COLUMNS} FROM server_metrics ORDER BY recorded_at DESC, id DESC LIMIT 1"), ()).await.map_err(row::error)?;
        let system_metric = sr
            .next()
            .await
            .map_err(row::error)?
            .map(|r| system(&r))
            .transpose()?;
        let mut ar = connection.query(&format!("SELECT {APP_COLUMNS} FROM app_metrics ORDER BY recorded_at DESC, id DESC LIMIT 1"), ()).await.map_err(row::error)?;
        let app_metric = ar
            .next()
            .await
            .map_err(row::error)?
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
        let connection = self.database.connect()?;
        let since = since.and_utc().timestamp_micros();
        let bucket = resolution_secs.max(1).saturating_mul(1_000_000);
        let system_sql = if resolution_secs > 10 {
            "SELECT avg(cpu_usage_percent), cast(avg(memory_used_bytes) as integer), cast(avg(memory_total_bytes) as integer), cast(avg(disk_used_bytes) as integer), cast(avg(disk_total_bytes) as integer), cast(avg(network_rx_bytes_delta) as integer), cast(avg(network_tx_bytes_delta) as integer), avg(load_avg_1m), avg(load_avg_5m), avg(load_avg_15m), recorded_at-(recorded_at%?2) FROM server_metrics WHERE recorded_at>=?1 GROUP BY recorded_at-(recorded_at%?2) ORDER BY 11".to_string()
        } else {
            format!(
                "SELECT {SYSTEM_COLUMNS} FROM server_metrics WHERE recorded_at>=?1 ORDER BY recorded_at LIMIT 10000"
            )
        };
        let mut sr = if resolution_secs > 10 {
            connection.query(&system_sql, params![since, bucket]).await
        } else {
            connection.query(&system_sql, params![since]).await
        }
        .map_err(row::error)?;
        let mut systems = Vec::new();
        while let Some(r) = sr.next().await.map_err(row::error)? {
            systems.push(system(&r)?);
        }
        let app_sql = if resolution_secs > 10 {
            "SELECT cast(avg(request_count) as integer),cast(avg(error_count) as integer),avg(avg_latency_ms),avg(p95_latency_ms),cast(avg(db_pool_active) as integer),cast(avg(db_pool_idle) as integer),cast(avg(zenoh_messages_in) as integer),cast(avg(zenoh_messages_out) as integer),recorded_at-(recorded_at%?2) FROM app_metrics WHERE recorded_at>=?1 GROUP BY recorded_at-(recorded_at%?2) ORDER BY 9".to_string()
        } else {
            format!(
                "SELECT {APP_COLUMNS} FROM app_metrics WHERE recorded_at>=?1 ORDER BY recorded_at LIMIT 10000"
            )
        };
        let mut ar = if resolution_secs > 10 {
            connection.query(&app_sql, params![since, bucket]).await
        } else {
            connection.query(&app_sql, params![since]).await
        }
        .map_err(row::error)?;
        let mut apps = Vec::new();
        while let Some(r) = ar.next().await.map_err(row::error)? {
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
        let mut writer = self.database.writer().await;
        let tx = writer.transaction().await.map_err(row::error)?;
        let cutoff = cutoff.and_utc().timestamp_micros();
        let system = tx
            .execute(
                "DELETE FROM server_metrics WHERE recorded_at<?1",
                params![cutoff],
            )
            .await
            .map_err(row::error)? as usize;
        let app = tx
            .execute(
                "DELETE FROM app_metrics WHERE recorded_at<?1",
                params![cutoff],
            )
            .await
            .map_err(row::error)? as usize;
        tx.commit().await.map_err(row::error)?;
        Ok((system, app))
    }
}
