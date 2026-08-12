use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use turso::{Connection, Row, params};

use crate::domains::operations::outbox_repository::OutboxRepository;
use crate::domains::operations::outbox_types::{OutboxEventRecord, OutboxSummaryRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

fn decode(record: &Row) -> Result<OutboxEventRecord, PersistenceError> {
    let payload: String = record.get(5).map_err(row::error)?;
    Ok(OutboxEventRecord {
        id: record.get(0).map_err(row::error)?,
        tenant_id: record.get(1).map_err(row::error)?,
        event_type: record.get(2).map_err(row::error)?,
        aggregate_type: record.get(3).map_err(row::error)?,
        aggregate_id: record.get(4).map_err(row::error)?,
        payload: serde_json::from_str(&payload)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        attempts: row::i32(record.get(6).map_err(row::error)?, "outbox.attempts")?,
        max_attempts: row::i32(record.get(7).map_err(row::error)?, "outbox.max_attempts")?,
        last_error: record.get(8).map_err(row::error)?,
        created_at: row::datetime(record.get(9).map_err(row::error)?)?.naive_utc(),
        updated_at: row::datetime(record.get(10).map_err(row::error)?)?.naive_utc(),
    })
}

async fn by_id(connection: &Connection, id: &str) -> Result<OutboxEventRecord, PersistenceError> {
    let mut rows = connection.query("SELECT id, tenant_id, event_type, aggregate_type, aggregate_id, payload, attempts, max_attempts, last_error, created_at, updated_at FROM rule_action_outbox WHERE id = ?1", params![id]).await.map_err(row::error)?;
    let record = rows
        .next()
        .await
        .map_err(row::error)?
        .ok_or(PersistenceError::NotFound)?;
    decode(&record)
}

#[async_trait]
impl OutboxRepository for TursoAdapter {
    async fn claim_batch(
        &self,
        worker_id: &str,
        limit: i64,
        lease_timeout: Duration,
    ) -> Result<Vec<OutboxEventRecord>, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let now = Utc::now().timestamp_micros();
        let lease_micros = i64::try_from(lease_timeout.as_micros()).unwrap_or(i64::MAX);
        let lease_cutoff = now.saturating_sub(lease_micros);
        transaction.execute("UPDATE rule_action_outbox SET status = 'dead_letter', locked_at = NULL, locked_by = NULL, last_error = 'delivery lease expired after final attempt', updated_at = ?2 WHERE status = 'processing' AND locked_at <= ?1 AND attempts >= max_attempts", params![lease_cutoff, now]).await.map_err(row::error)?;
        let mut rows = transaction.query(
            "WITH ranked AS (SELECT id, row_number() OVER (PARTITION BY tenant_id ORDER BY available_at, created_at, id) tenant_rank, available_at, created_at FROM rule_action_outbox WHERE attempts < max_attempts AND ((status IN ('pending','failed') AND available_at <= ?1) OR (status = 'processing' AND locked_at <= ?2))) SELECT id FROM ranked ORDER BY tenant_rank, available_at, created_at, id LIMIT ?3",
            params![now, lease_cutoff, limit],
        ).await.map_err(row::error)?;
        let mut ids = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            ids.push(record.get::<String>(0).map_err(row::error)?);
        }
        drop(rows);
        let mut events = Vec::new();
        for id in ids {
            transaction.execute("UPDATE rule_action_outbox SET status = 'processing', attempts = attempts + 1, locked_at = ?2, locked_by = ?3, updated_at = ?2 WHERE id = ?1", params![id.clone(), now, worker_id]).await.map_err(row::error)?;
            events.push(by_id(&transaction, &id).await?);
        }
        transaction.commit().await.map_err(row::error)?;
        Ok(events)
    }

    async fn mark_succeeded(
        &self,
        event_id: &str,
        worker_id: &str,
    ) -> Result<bool, PersistenceError> {
        let writer = self.database.writer().await;
        writer.execute("UPDATE rule_action_outbox SET status = 'succeeded', locked_at = NULL, locked_by = NULL, last_error = NULL, updated_at = ?3 WHERE id = ?1 AND status = 'processing' AND locked_by = ?2", params![event_id, worker_id, Utc::now().timestamp_micros()]).await.map(|count| count == 1).map_err(row::error)
    }

    async fn mark_failed(
        &self,
        event_id: &str,
        worker_id: &str,
        attempts: i32,
        max_attempts: i32,
        error: &str,
    ) -> Result<bool, PersistenceError> {
        let terminal = attempts >= max_attempts;
        let status = if terminal { "dead_letter" } else { "failed" };
        let now = Utc::now().timestamp_micros();
        let delay = 2_i64.pow(attempts.clamp(1, 8) as u32).min(300) * 1_000_000;
        let writer = self.database.writer().await;
        writer.execute("UPDATE rule_action_outbox SET status = ?4, available_at = ?5, locked_at = NULL, locked_by = NULL, last_error = ?6, updated_at = ?7 WHERE id = ?1 AND status = 'processing' AND locked_by = ?2 AND attempts = ?3", params![event_id, worker_id, i64::from(attempts), status, now.saturating_add(delay), error.chars().take(2000).collect::<String>(), now]).await.map(|count| count == 1).map_err(row::error)
    }

    async fn summary(&self, tenant: &TenantId) -> Result<OutboxSummaryRecord, PersistenceError> {
        let connection = self.database.connect()?;
        let now = Utc::now().timestamp_micros();
        let mut rows = connection.query("SELECT count(*) FILTER (WHERE status='pending'), count(*) FILTER (WHERE status='processing'), count(*) FILTER (WHERE status='failed'), count(*) FILTER (WHERE status='dead_letter'), count(*) FILTER (WHERE status='succeeded'), min(created_at) FILTER (WHERE status IN ('pending','failed')) FROM rule_action_outbox WHERE tenant_id = ?1", params![tenant.as_str()]).await.map_err(row::error)?;
        let record = rows
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?;
        let oldest_us: Option<i64> = record.get(5).map_err(row::error)?;
        Ok(OutboxSummaryRecord {
            pending_count: record.get(0).map_err(row::error)?,
            processing_count: record.get(1).map_err(row::error)?,
            failed_count: record.get(2).map_err(row::error)?,
            dead_letter_count: record.get(3).map_err(row::error)?,
            succeeded_count: record.get(4).map_err(row::error)?,
            oldest_pending_at: oldest_us
                .map(row::datetime)
                .transpose()?
                .map(|value| value.naive_utc()),
            oldest_pending_age_seconds: oldest_us
                .map(|value| now.saturating_sub(value) / 1_000_000),
        })
    }

    async fn list_dead_letters(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OutboxEventRecord>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection.query("SELECT id, tenant_id, event_type, aggregate_type, aggregate_id, payload, attempts, max_attempts, last_error, created_at, updated_at FROM rule_action_outbox WHERE tenant_id = ?1 AND status = 'dead_letter' ORDER BY updated_at DESC, id DESC LIMIT ?2 OFFSET ?3", params![tenant.as_str(), limit, offset]).await.map_err(row::error)?;
        let mut events = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            events.push(decode(&record)?);
        }
        Ok(events)
    }

    async fn replay_dead_letters(
        &self,
        tenant: &TenantId,
        event_ids: Option<Vec<String>>,
    ) -> Result<usize, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let now = Utc::now().timestamp_micros();
        let mut count = 0;
        if let Some(ids) = event_ids {
            for id in ids {
                count += transaction.execute("UPDATE rule_action_outbox SET status='pending', attempts=0, available_at=?3, locked_at=NULL, locked_by=NULL, last_error=NULL, updated_at=?3 WHERE tenant_id=?1 AND id=?2 AND status='dead_letter'", params![tenant.as_str(), id, now]).await.map_err(row::error)? as usize;
            }
        } else {
            count = transaction.execute("UPDATE rule_action_outbox SET status='pending', attempts=0, available_at=?2, locked_at=NULL, locked_by=NULL, last_error=NULL, updated_at=?2 WHERE tenant_id=?1 AND status='dead_letter'", params![tenant.as_str(), now]).await.map_err(row::error)? as usize;
        }
        transaction.commit().await.map_err(row::error)?;
        Ok(count)
    }
}
