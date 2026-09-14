use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::connection::Connection;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Jsonb, Nullable, Text, Timestamptz};
use serde_json::Value;

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::device_blueprints::DeviceBlueprintRepository;
use extrittio_backend_core::device_blueprints::{
    BlueprintDraftRecord, BlueprintList, BlueprintRecord, BlueprintRevisionRecord,
    CreateBlueprintRecord, PublishBlueprintOutcome, PublishBlueprintRecord,
    ReplaceBlueprintDraftRecord,
};

use crate::error::map_diesel_error;
use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresDeviceBlueprintRepository {
    executor: PostgresExecutor,
}
impl PostgresDeviceBlueprintRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}

#[derive(QueryableByName)]
struct BlueprintRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    blueprint_key: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Integer>)]
    latest_revision: Option<i32>,
    #[diesel(sql_type = Timestamptz)]
    created_at: DateTime<Utc>,
    #[diesel(sql_type = Timestamptz)]
    updated_at: DateTime<Utc>,
}

#[derive(QueryableByName)]
struct DraftRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    blueprint_id: String,
    #[diesel(sql_type = Jsonb)]
    document: Value,
    #[diesel(sql_type = Timestamptz)]
    created_at: DateTime<Utc>,
    #[diesel(sql_type = Timestamptz)]
    updated_at: DateTime<Utc>,
}

#[derive(QueryableByName, Clone)]
struct RevisionRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    blueprint_id: String,
    #[diesel(sql_type = Integer)]
    revision: i32,
    #[diesel(sql_type = Jsonb)]
    document: Value,
    #[diesel(sql_type = Text)]
    document_hash: String,
    #[diesel(sql_type = Jsonb)]
    compatibility: Value,
    #[diesel(sql_type = Timestamptz)]
    created_at: DateTime<Utc>,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    total: i64,
}

fn blueprint(record: BlueprintRow) -> BlueprintRecord {
    BlueprintRecord {
        id: record.id,
        key: record.blueprint_key,
        name: record.name,
        description: record.description,
        latest_revision: record.latest_revision,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn draft(record: DraftRow) -> BlueprintDraftRecord {
    BlueprintDraftRecord {
        id: record.id,
        blueprint_id: record.blueprint_id,
        document: record.document,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn revision(record: RevisionRow) -> BlueprintRevisionRecord {
    BlueprintRevisionRecord {
        id: record.id,
        blueprint_id: record.blueprint_id,
        revision: record.revision,
        document: record.document,
        document_hash: record.document_hash,
        compatibility: record.compatibility,
        created_at: record.created_at,
    }
}

const BLUEPRINT_SELECT: &str = "SELECT b.id, b.blueprint_key, b.name, b.description,
            (SELECT max(r.revision) FROM device_blueprint_revisions r
             WHERE r.tenant_id = b.tenant_id AND r.blueprint_id = b.id) AS latest_revision,
            b.created_at, b.updated_at
     FROM device_blueprints b";

const DRAFT_SELECT: &str = "SELECT id, blueprint_id, document, created_at, updated_at
     FROM device_blueprint_drafts";

const REVISION_SELECT: &str =
    "SELECT id, blueprint_id, revision, document, document_hash, compatibility, created_at
     FROM device_blueprint_revisions";

#[async_trait]
impl DeviceBlueprintRepository for PostgresDeviceBlueprintRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<BlueprintList, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                let total = diesel::sql_query(
                    "SELECT count(*) AS total FROM device_blueprints WHERE tenant_id = $1",
                )
                .bind::<Text, _>(&tenant_id)
                .get_result::<CountRow>(connection)
                .map_err(map_diesel_error)?
                .total;
                let query = format!(
                    "{BLUEPRINT_SELECT} WHERE b.tenant_id = $1
                     ORDER BY b.name, b.id LIMIT $2 OFFSET $3"
                );
                let records = diesel::sql_query(query)
                    .bind::<Text, _>(&tenant_id)
                    .bind::<BigInt, _>(limit)
                    .bind::<BigInt, _>(offset)
                    .load::<BlueprintRow>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(blueprint)
                    .collect();
                Ok(BlueprintList { records, total })
            })
            .await
    }

    async fn get(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let blueprint_id = blueprint_id.to_string();
        self.executor
            .run(move |connection| {
                diesel::sql_query(format!(
                    "{BLUEPRINT_SELECT} WHERE b.tenant_id = $1 AND b.id = $2"
                ))
                .bind::<Text, _>(tenant_id)
                .bind::<Text, _>(blueprint_id)
                .get_result::<BlueprintRow>(connection)
                .optional()
                .map(|record| record.map(blueprint))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateBlueprintRecord,
    ) -> Result<(BlueprintRecord, BlueprintDraftRecord), PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        diesel::sql_query(
                            "INSERT INTO device_blueprints
                             (id, tenant_id, blueprint_key, name, description, created_at, updated_at)
                             VALUES ($1, $2, $3, $4, $5, $6, $6)",
                        )
                        .bind::<Text, _>(&record.id)
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&record.key)
                        .bind::<Text, _>(&record.name)
                        .bind::<Nullable<Text>, _>(&record.description)
                        .bind::<Timestamptz, _>(record.now)
                        .execute(connection)?;
                        diesel::sql_query(
                            "INSERT INTO device_blueprint_drafts
                             (id, tenant_id, blueprint_id, document, created_at, updated_at)
                             VALUES ($1, $2, $3, $4, $5, $5)",
                        )
                        .bind::<Text, _>(&record.draft_id)
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&record.id)
                        .bind::<Jsonb, _>(&record.document)
                        .bind::<Timestamptz, _>(record.now)
                        .execute(connection)?;
                        let blueprint_row = diesel::sql_query(format!(
                            "{BLUEPRINT_SELECT} WHERE b.tenant_id = $1 AND b.id = $2"
                        ))
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&record.id)
                        .get_result::<BlueprintRow>(connection)?;
                        let draft_row = diesel::sql_query(format!(
                            "{DRAFT_SELECT} WHERE tenant_id = $1 AND blueprint_id = $2"
                        ))
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&record.id)
                        .get_result::<DraftRow>(connection)?;
                        Ok((blueprint(blueprint_row), draft(draft_row)))
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_draft(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintDraftRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let blueprint_id = blueprint_id.to_string();
        self.executor
            .run(move |connection| {
                diesel::sql_query(format!(
                    "{DRAFT_SELECT} WHERE tenant_id = $1 AND blueprint_id = $2"
                ))
                .bind::<Text, _>(tenant_id)
                .bind::<Text, _>(blueprint_id)
                .get_result::<DraftRow>(connection)
                .optional()
                .map(|record| record.map(draft))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn replace_draft(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
        record: ReplaceBlueprintDraftRecord,
    ) -> Result<Option<BlueprintDraftRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let blueprint_id = blueprint_id.to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let updated = diesel::sql_query(
                            "UPDATE device_blueprints SET blueprint_key = $3, name = $4,
                             description = $5, updated_at = $6
                             WHERE tenant_id = $1 AND id = $2",
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&blueprint_id)
                        .bind::<Text, _>(&record.key)
                        .bind::<Text, _>(&record.name)
                        .bind::<Nullable<Text>, _>(&record.description)
                        .bind::<Timestamptz, _>(record.now)
                        .execute(connection)?;
                        if updated == 0 {
                            return Ok(None);
                        }
                        let row = diesel::sql_query(
                            "UPDATE device_blueprint_drafts SET document = $3, updated_at = $4
                             WHERE tenant_id = $1 AND blueprint_id = $2
                             RETURNING id, blueprint_id, document, created_at, updated_at",
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&blueprint_id)
                        .bind::<Jsonb, _>(&record.document)
                        .bind::<Timestamptz, _>(record.now)
                        .get_result::<DraftRow>(connection)?;
                        Ok(Some(draft(row)))
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn latest_revision(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
    ) -> Result<Option<BlueprintRevisionRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let blueprint_id = blueprint_id.to_string();
        self.executor
            .run(move |connection| {
                diesel::sql_query(format!(
                    "{REVISION_SELECT} WHERE tenant_id = $1 AND blueprint_id = $2
                     ORDER BY revision DESC LIMIT 1"
                ))
                .bind::<Text, _>(tenant_id)
                .bind::<Text, _>(blueprint_id)
                .get_result::<RevisionRow>(connection)
                .optional()
                .map(|record| record.map(revision))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_revision(
        &self,
        tenant: &TenantId,
        revision_id: &str,
    ) -> Result<Option<BlueprintRevisionRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let revision_id = revision_id.to_string();
        self.executor
            .run(move |connection| {
                diesel::sql_query(format!(
                    "{REVISION_SELECT} WHERE tenant_id = $1 AND id = $2"
                ))
                .bind::<Text, _>(tenant_id)
                .bind::<Text, _>(revision_id)
                .get_result::<RevisionRow>(connection)
                .optional()
                .map(|record| record.map(revision))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn publish(
        &self,
        tenant: &TenantId,
        blueprint_id: &str,
        record: PublishBlueprintRecord,
    ) -> Result<PublishBlueprintOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let blueprint_id = blueprint_id.to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        // Use the same parent-before-draft lock order as draft replacement.
                        let parent = diesel::sql_query(format!(
                            "{BLUEPRINT_SELECT} WHERE b.tenant_id = $1 AND b.id = $2 FOR UPDATE OF b"
                        ))
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&blueprint_id)
                        .get_result::<BlueprintRow>(connection)
                        .optional()?;
                        if parent.is_none() { return Ok(PublishBlueprintOutcome::BlueprintNotFound); }
                        let draft_row = diesel::sql_query(format!(
                            "{DRAFT_SELECT} WHERE tenant_id = $1 AND blueprint_id = $2 FOR UPDATE"
                        ))
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&blueprint_id)
                        .get_result::<DraftRow>(connection)
                        .optional()?;
                        let Some(draft_row) = draft_row else {
                            return Ok(PublishBlueprintOutcome::BlueprintNotFound);
                        };
                        if draft_row.updated_at != record.expected_draft_updated_at || draft_row.document != record.document {
                            return Ok(PublishBlueprintOutcome::DraftChanged);
                        }
                        let latest = diesel::sql_query(format!(
                            "{REVISION_SELECT} WHERE tenant_id = $1 AND blueprint_id = $2 ORDER BY revision DESC LIMIT 1"
                        ))
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&blueprint_id)
                        .get_result::<RevisionRow>(connection)
                        .optional()?;
                        if let Some(ref latest) = latest {
                            if latest.document_hash == record.document_hash && latest.document == record.document {
                                return Ok(PublishBlueprintOutcome::Published(revision(latest.clone())));
                            }
                        }
                        if latest.as_ref().map(|value| &value.id) != record.expected_previous_revision_id.as_ref() {
                            return Ok(PublishBlueprintOutcome::PublicationChanged);
                        }
                        let next_revision = diesel::sql_query(
                            "SELECT (COALESCE(max(revision), 0) + 1)::bigint AS total
                             FROM device_blueprint_revisions
                             WHERE tenant_id = $1 AND blueprint_id = $2",
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&blueprint_id)
                        .get_result::<CountRow>(connection)?
                        .total;
                        let next_revision: i32 = next_revision
                            .try_into()
                            .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                        let row = diesel::sql_query(
                            "INSERT INTO device_blueprint_revisions
                             (id, tenant_id, blueprint_id, revision, document, document_hash,
                              compatibility, created_at)
                             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                             RETURNING id, blueprint_id, revision, document, document_hash,
                                       compatibility, created_at",
                        )
                        .bind::<Text, _>(&record.revision_id)
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&blueprint_id)
                        .bind::<Integer, _>(next_revision)
                        .bind::<Jsonb, _>(&record.document)
                        .bind::<Text, _>(&record.document_hash)
                        .bind::<Jsonb, _>(&record.compatibility)
                        .bind::<Timestamptz, _>(record.now)
                        .get_result::<RevisionRow>(connection)?;
                        diesel::sql_query(
                            "UPDATE device_blueprints SET updated_at = $3
                             WHERE tenant_id = $1 AND id = $2",
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&blueprint_id)
                        .bind::<Timestamptz, _>(record.now)
                        .execute(connection)?;
                        Ok(PublishBlueprintOutcome::Published(revision(row)))
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }
}
