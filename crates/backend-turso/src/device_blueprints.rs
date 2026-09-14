use async_trait::async_trait;
use turso::{Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::device_blueprints::DeviceBlueprintRepository;
use extrittio_backend_core::device_blueprints::{
    BlueprintDraftRecord, BlueprintList, BlueprintRecord, BlueprintRevisionRecord,
    CreateBlueprintRecord, PublishBlueprintOutcome, PublishBlueprintRecord,
    ReplaceBlueprintDraftRecord,
};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoDeviceBlueprintRepository {
    handles: TursoConnectionHandles,
}
impl TursoDeviceBlueprintRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}

fn json(value: &serde_json::Value) -> Result<String, PersistenceError> {
    serde_json::to_string(value).map_err(|error| PersistenceError::Internal(error.to_string()))
}

fn parse_json(value: String) -> Result<serde_json::Value, PersistenceError> {
    serde_json::from_str(&value).map_err(|error| PersistenceError::CorruptData(error.to_string()))
}

fn blueprint(record: &Row) -> Result<BlueprintRecord, PersistenceError> {
    Ok(BlueprintRecord {
        id: record.get(0).map_err(row::legacy_error)?,
        key: record.get(1).map_err(row::legacy_error)?,
        name: record.get(2).map_err(row::legacy_error)?,
        description: record.get(3).map_err(row::legacy_error)?,
        latest_revision: record
            .get::<Option<i64>>(4)
            .map_err(row::legacy_error)?
            .map(|value| row::i32(value, "device_blueprint_revisions.revision"))
            .transpose()?,
        created_at: row::datetime(record.get(5).map_err(row::legacy_error)?)?,
        updated_at: row::datetime(record.get(6).map_err(row::legacy_error)?)?,
    })
}

fn draft(record: &Row) -> Result<BlueprintDraftRecord, PersistenceError> {
    Ok(BlueprintDraftRecord {
        id: record.get(0).map_err(row::legacy_error)?,
        blueprint_id: record.get(1).map_err(row::legacy_error)?,
        document: parse_json(record.get(2).map_err(row::legacy_error)?)?,
        created_at: row::datetime(record.get(3).map_err(row::legacy_error)?)?,
        updated_at: row::datetime(record.get(4).map_err(row::legacy_error)?)?,
    })
}

fn revision(record: &Row) -> Result<BlueprintRevisionRecord, PersistenceError> {
    Ok(BlueprintRevisionRecord {
        id: record.get(0).map_err(row::legacy_error)?,
        blueprint_id: record.get(1).map_err(row::legacy_error)?,
        revision: row::i32(
            record.get(2).map_err(row::legacy_error)?,
            "device_blueprint_revisions.revision",
        )?,
        document: parse_json(record.get(3).map_err(row::legacy_error)?)?,
        document_hash: record.get(4).map_err(row::legacy_error)?,
        compatibility: parse_json(record.get(5).map_err(row::legacy_error)?)?,
        created_at: row::datetime(record.get(6).map_err(row::legacy_error)?)?,
    })
}

const BLUEPRINT_SELECT: &str = "SELECT b.id, b.blueprint_key, b.name, b.description,
            (SELECT max(r.revision) FROM device_blueprint_revisions r
             WHERE r.tenant_id = b.tenant_id AND r.blueprint_id = b.id),
            b.created_at, b.updated_at
     FROM device_blueprints b";
const DRAFT_SELECT: &str =
    "SELECT id, blueprint_id, document, created_at, updated_at FROM device_blueprint_drafts";
const REVISION_SELECT: &str =
    "SELECT id, blueprint_id, revision, document, document_hash, compatibility, created_at
     FROM device_blueprint_revisions";

#[async_trait]
impl DeviceBlueprintRepository for TursoDeviceBlueprintRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<BlueprintList, PersistenceError> {
        let connection = self.connect()?;
        let mut count_rows = connection
            .query(
                "SELECT count(*) FROM device_blueprints WHERE tenant_id = ?1",
                params![tenant.as_str()],
            )
            .await
            .map_err(row::legacy_error)?;
        let total = count_rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::legacy_error)?;
        drop(count_rows);
        let mut rows = connection
            .query(
                &format!(
                    "{BLUEPRINT_SELECT} WHERE b.tenant_id = ?1
                     ORDER BY b.name, b.id LIMIT ?2 OFFSET ?3"
                ),
                params![tenant.as_str(), limit, offset],
            )
            .await
            .map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            records.push(blueprint(&record)?);
        }
        Ok(BlueprintList { records, total })
    }

    async fn get(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                &format!("{BLUEPRINT_SELECT} WHERE b.tenant_id = ?1 AND b.id = ?2"),
                params![tenant.as_str(), blueprint_id],
            )
            .await
            .map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| blueprint(&record))
            .transpose()
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateBlueprintRecord,
    ) -> Result<(BlueprintRecord, BlueprintDraftRecord), PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        transaction
            .execute(
                "INSERT INTO device_blueprints
                 (id, tenant_id, blueprint_key, name, description, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    record.id.clone(),
                    tenant.as_str(),
                    record.key,
                    record.name,
                    record.description,
                    record.now.timestamp_micros()
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        transaction
            .execute(
                "INSERT INTO device_blueprint_drafts
                 (id, tenant_id, blueprint_id, document, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![
                    record.draft_id,
                    tenant.as_str(),
                    record.id.clone(),
                    json(&record.document)?,
                    record.now.timestamp_micros()
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let mut blueprint_rows = transaction
            .query(
                &format!("{BLUEPRINT_SELECT} WHERE b.tenant_id = ?1 AND b.id = ?2"),
                params![tenant.as_str(), record.id.clone()],
            )
            .await
            .map_err(row::legacy_error)?;
        let blueprint_record = blueprint(
            &blueprint_rows
                .next()
                .await
                .map_err(row::legacy_error)?
                .ok_or(PersistenceError::NotFound)?,
        )?;
        drop(blueprint_rows);
        let mut draft_rows = transaction
            .query(
                &format!("{DRAFT_SELECT} WHERE tenant_id = ?1 AND blueprint_id = ?2"),
                params![tenant.as_str(), record.id],
            )
            .await
            .map_err(row::legacy_error)?;
        let draft_record = draft(
            &draft_rows
                .next()
                .await
                .map_err(row::legacy_error)?
                .ok_or(PersistenceError::NotFound)?,
        )?;
        drop(draft_rows);
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok((blueprint_record, draft_record))
    }

    async fn get_draft(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintDraftRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                &format!("{DRAFT_SELECT} WHERE tenant_id = ?1 AND blueprint_id = ?2"),
                params![tenant.as_str(), blueprint_id],
            )
            .await
            .map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| draft(&record))
            .transpose()
    }

    async fn replace_draft(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
        record: ReplaceBlueprintDraftRecord,
    ) -> Result<Option<BlueprintDraftRecord>, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let updated = transaction
            .execute(
                "UPDATE device_blueprints SET blueprint_key = ?3, name = ?4,
                 description = ?5, updated_at = ?6 WHERE tenant_id = ?1 AND id = ?2",
                params![
                    tenant.as_str(),
                    blueprint_id,
                    record.key,
                    record.name,
                    record.description,
                    record.now.timestamp_micros()
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        if updated == 0 {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(None);
        }
        let mut rows = transaction
            .query(
                "UPDATE device_blueprint_drafts SET document = ?3, updated_at = ?4
                 WHERE tenant_id = ?1 AND blueprint_id = ?2
                 RETURNING id, blueprint_id, document, created_at, updated_at",
                params![
                    tenant.as_str(),
                    blueprint_id,
                    json(&record.document)?,
                    record.now.timestamp_micros()
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let record = draft(
            &rows
                .next()
                .await
                .map_err(row::legacy_error)?
                .ok_or(PersistenceError::NotFound)?,
        )?;
        drop(rows);
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(Some(record))
    }

    async fn latest_revision(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintRevisionRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                &format!(
                    "{REVISION_SELECT} WHERE tenant_id = ?1 AND blueprint_id = ?2
                     ORDER BY revision DESC LIMIT 1"
                ),
                params![tenant.as_str(), blueprint_id],
            )
            .await
            .map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| revision(&record))
            .transpose()
    }

    async fn get_revision(
        &self,
        tenant: &TenantId,
        revision_id: &str,
    ) -> Result<Option<BlueprintRevisionRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                &format!("{REVISION_SELECT} WHERE tenant_id = ?1 AND id = ?2"),
                params![tenant.as_str(), revision_id],
            )
            .await
            .map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| revision(&record))
            .transpose()
    }

    async fn publish(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
        record: PublishBlueprintRecord,
    ) -> Result<PublishBlueprintOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let mut draft_rows = transaction
            .query(
                &format!("{DRAFT_SELECT} WHERE tenant_id = ?1 AND blueprint_id = ?2"),
                params![tenant.as_str(), blueprint_id],
            )
            .await
            .map_err(row::legacy_error)?;
        let draft_record = draft_rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .map(|value| draft(&value))
            .transpose()?;
        drop(draft_rows);
        let Some(draft_record) = draft_record else {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(PublishBlueprintOutcome::BlueprintNotFound);
        };
        if draft_record.updated_at != record.expected_draft_updated_at
            || draft_record.document != record.document
        {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(PublishBlueprintOutcome::DraftChanged);
        }
        let mut latest_rows = transaction.query(
            &format!("{REVISION_SELECT} WHERE tenant_id = ?1 AND blueprint_id = ?2 ORDER BY revision DESC LIMIT 1"),
            params![tenant.as_str(), blueprint_id],
        ).await.map_err(row::legacy_error)?;
        let latest = latest_rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .map(|value| revision(&value))
            .transpose()?;
        drop(latest_rows);
        if let Some(ref latest) = latest {
            if latest.document_hash == record.document_hash && latest.document == record.document {
                transaction.rollback().await.map_err(row::legacy_error)?;
                return Ok(PublishBlueprintOutcome::Published(latest.clone()));
            }
        }
        if latest.as_ref().map(|value| &value.id) != record.expected_previous_revision_id.as_ref() {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(PublishBlueprintOutcome::PublicationChanged);
        }
        let mut revision_rows = transaction
            .query(
                "SELECT COALESCE(max(revision), 0) + 1 FROM device_blueprint_revisions
                 WHERE tenant_id = ?1 AND blueprint_id = ?2",
                params![tenant.as_str(), blueprint_id],
            )
            .await
            .map_err(row::legacy_error)?;
        let next_revision: i64 = revision_rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::legacy_error)?;
        drop(revision_rows);
        let next_revision = row::i32(next_revision, "device_blueprint_revisions.revision")?;
        let mut rows = transaction
            .query(
                "INSERT INTO device_blueprint_revisions
                 (id, tenant_id, blueprint_id, revision, document, document_hash,
                  compatibility, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 RETURNING id, blueprint_id, revision, document, document_hash,
                           compatibility, created_at",
                params![
                    record.revision_id,
                    tenant.as_str(),
                    blueprint_id,
                    next_revision,
                    json(&record.document)?,
                    record.document_hash,
                    json(&record.compatibility)?,
                    record.now.timestamp_micros()
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let published = revision(
            &rows
                .next()
                .await
                .map_err(row::legacy_error)?
                .ok_or(PersistenceError::NotFound)?,
        )?;
        drop(rows);
        transaction
            .execute(
                "UPDATE device_blueprints SET updated_at = ?3 WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), blueprint_id, record.now.timestamp_micros()],
            )
            .await
            .map_err(row::legacy_error)?;
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(PublishBlueprintOutcome::Published(published))
    }
}
