//! PostgreSQL implementation of the core zone ports.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::Connection;
use diesel::OptionalExtension;
use diesel::RunQueryDsl;
use diesel::prelude::QueryableByName;
use diesel::sql_query;
use diesel::sql_types::{Bool, Jsonb, Nullable, Text, Timestamptz};
use extrittio_backend_core::zones::{
    DeleteZoneOutcome, NewZone, RuleZoneSnapshotRepository, Zone, ZonePatch, ZoneRepository,
};
use extrittio_backend_core::{PersistenceError, TenantId};
use serde_json::Value;

use crate::error::map_diesel_error;
use crate::{PostgresExecutor, PostgresPool};

#[cfg(test)]
const ZONE_COLUMNS: &str =
    "id, tenant_id, name, description, geometry_type, geometry_json, color, created_at, updated_at";

const LIST_SQL: &str = r#"
    SELECT id, tenant_id, name, description, geometry_type, geometry_json, color,
           created_at, updated_at
      FROM zones
     WHERE tenant_id = $1
     ORDER BY name COLLATE "C" ASC, id COLLATE "C" ASC
"#;

const LIST_FOR_RULE_SNAPSHOT_SQL: &str = r#"
    SELECT id, tenant_id, name, description, geometry_type, geometry_json, color,
           created_at, updated_at
      FROM zones
     ORDER BY tenant_id COLLATE "C" ASC,
              name COLLATE "C" ASC,
              id COLLATE "C" ASC
"#;

const GET_SQL: &str = r#"
    SELECT id, tenant_id, name, description, geometry_type, geometry_json, color,
           created_at, updated_at
      FROM zones
     WHERE tenant_id = $1 AND id = $2
"#;

const CREATE_SQL: &str = r#"
    INSERT INTO zones (
        id, tenant_id, name, description, geometry_type, geometry_json, color
    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
    RETURNING id, tenant_id, name, description, geometry_type, geometry_json, color,
              created_at, updated_at
"#;

const UPDATE_SQL: &str = r#"
    UPDATE zones
       SET name = COALESCE($3, name),
           description = COALESCE($4, description),
           geometry_type = COALESCE($5, geometry_type),
           geometry_json = COALESCE($6, geometry_json),
           color = COALESCE($7, color),
           updated_at = $8
     WHERE tenant_id = $1 AND id = $2
    RETURNING id, tenant_id, name, description, geometry_type, geometry_json, color,
              created_at, updated_at
"#;

const LOCK_FOR_DELETE_SQL: &str = r#"
    SELECT id
      FROM zones
     WHERE tenant_id = $1 AND id = $2
     FOR UPDATE
"#;

const IS_IN_USE_SQL: &str = r#"
    SELECT EXISTS (
        SELECT 1
          FROM rule_conditions
         WHERE tenant_id = $1 AND zone_id = $2
    ) AS present
"#;

const DELETE_SQL: &str = "DELETE FROM zones WHERE tenant_id = $1 AND id = $2";

#[derive(Debug, QueryableByName)]
struct ZoneRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    tenant_id: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    description: String,
    #[diesel(sql_type = Text)]
    geometry_type: String,
    #[diesel(sql_type = Jsonb)]
    geometry_json: Value,
    #[diesel(sql_type = Text)]
    color: String,
    #[diesel(sql_type = Timestamptz)]
    created_at: DateTime<Utc>,
    #[diesel(sql_type = Timestamptz)]
    updated_at: DateTime<Utc>,
}

impl ZoneRow {
    fn into_domain(self) -> Result<Zone, PersistenceError> {
        let tenant_id = TenantId::new(self.tenant_id)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?;
        Ok(Zone {
            id: self.id,
            tenant_id,
            name: self.name,
            description: self.description,
            geometry_type: self.geometry_type,
            geometry_json: self.geometry_json,
            color: self.color,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(QueryableByName)]
struct LockedZone {
    #[diesel(sql_type = Text)]
    #[diesel(column_name = id)]
    _id: String,
}

#[derive(QueryableByName)]
struct Presence {
    #[diesel(sql_type = Bool)]
    present: bool,
}

/// The PostgreSQL zone adapter. Clones share the same bounded connection pool.
#[derive(Clone)]
pub struct PostgresZoneRepository {
    executor: PostgresExecutor,
}

impl PostgresZoneRepository {
    #[must_use]
    pub fn new(executor: PostgresExecutor) -> Self {
        Self { executor }
    }

    #[must_use]
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self::new(PostgresExecutor::new(pool))
    }

    #[must_use]
    pub fn executor(&self) -> &PostgresExecutor {
        &self.executor
    }
}

#[async_trait]
impl ZoneRepository for PostgresZoneRepository {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<Zone>, PersistenceError> {
        let tenant = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                sql_query(LIST_SQL)
                    .bind::<Text, _>(tenant)
                    .load::<ZoneRow>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(ZoneRow::into_domain)
                    .collect()
            })
            .await
    }

    async fn get(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<Option<Zone>, PersistenceError> {
        let tenant = tenant.as_str().to_owned();
        let zone_id = zone_id.to_owned();
        self.executor
            .run(move |connection| {
                sql_query(GET_SQL)
                    .bind::<Text, _>(tenant)
                    .bind::<Text, _>(zone_id)
                    .get_result::<ZoneRow>(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                    .map(ZoneRow::into_domain)
                    .transpose()
            })
            .await
    }

    async fn create(&self, tenant: &TenantId, zone: NewZone) -> Result<Zone, PersistenceError> {
        let tenant = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                sql_query(CREATE_SQL)
                    .bind::<Text, _>(zone.id)
                    .bind::<Text, _>(tenant)
                    .bind::<Text, _>(zone.name)
                    .bind::<Text, _>(zone.description)
                    .bind::<Text, _>(zone.geometry_type)
                    .bind::<Jsonb, _>(zone.geometry_json)
                    .bind::<Text, _>(zone.color)
                    .get_result::<ZoneRow>(connection)
                    .map_err(map_diesel_error)?
                    .into_domain()
            })
            .await
    }

    async fn update(
        &self,
        tenant: &TenantId,
        zone_id: &str,
        patch: ZonePatch,
    ) -> Result<Option<Zone>, PersistenceError> {
        let tenant = tenant.as_str().to_owned();
        let zone_id = zone_id.to_owned();
        self.executor
            .run(move |connection| {
                sql_query(UPDATE_SQL)
                    .bind::<Text, _>(tenant)
                    .bind::<Text, _>(zone_id)
                    .bind::<Nullable<Text>, _>(patch.name)
                    .bind::<Nullable<Text>, _>(patch.description)
                    .bind::<Nullable<Text>, _>(patch.geometry_type)
                    .bind::<Nullable<Jsonb>, _>(patch.geometry_json)
                    .bind::<Nullable<Text>, _>(patch.color)
                    .bind::<Timestamptz, _>(patch.updated_at)
                    .get_result::<ZoneRow>(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                    .map(ZoneRow::into_domain)
                    .transpose()
            })
            .await
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<DeleteZoneOutcome, PersistenceError> {
        let tenant = tenant.as_str().to_owned();
        let zone_id = zone_id.to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let locked = sql_query(LOCK_FOR_DELETE_SQL)
                            .bind::<Text, _>(&tenant)
                            .bind::<Text, _>(&zone_id)
                            .get_result::<LockedZone>(connection)
                            .optional()?;
                        if locked.is_none() {
                            return Ok(DeleteZoneOutcome::NotFound);
                        }

                        let in_use = sql_query(IS_IN_USE_SQL)
                            .bind::<Text, _>(&tenant)
                            .bind::<Text, _>(&zone_id)
                            .get_result::<Presence>(connection)?
                            .present;
                        if in_use {
                            return Ok(DeleteZoneOutcome::InUse);
                        }

                        sql_query(DELETE_SQL)
                            .bind::<Text, _>(&tenant)
                            .bind::<Text, _>(&zone_id)
                            .execute(connection)?;
                        Ok(DeleteZoneOutcome::Deleted)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }
}

#[async_trait]
impl RuleZoneSnapshotRepository for PostgresZoneRepository {
    async fn list_for_rule_snapshot(&self) -> Result<Vec<Zone>, PersistenceError> {
        self.executor
            .run(move |connection| {
                sql_query(LIST_FOR_RULE_SNAPSHOT_SQL)
                    .load::<ZoneRow>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(ZoneRow::into_domain)
                    .collect()
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::*;

    fn row(tenant_id: &str) -> ZoneRow {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 8, 31, 12, 0, 0)
            .single()
            .expect("valid timestamp");
        ZoneRow {
            id: "zone-1".into(),
            tenant_id: tenant_id.into(),
            name: "Lab".into(),
            description: String::new(),
            geometry_type: "circle".into(),
            geometry_json: json!({"center": [1.0, 2.0], "radius_meters": 3.0}),
            color: "#4A90D9".into(),
            created_at: timestamp,
            updated_at: timestamp,
        }
    }

    #[test]
    fn row_translation_validates_the_tenant_identifier() {
        assert_eq!(
            row("tenant-a")
                .into_domain()
                .expect("valid zone")
                .tenant_id
                .as_str(),
            "tenant-a"
        );
        assert!(matches!(
            row(" ").into_domain(),
            Err(PersistenceError::CorruptData(_))
        ));
    }

    #[test]
    fn tenant_crud_queries_cannot_omit_tenant_scope() {
        for sql in [
            LIST_SQL,
            GET_SQL,
            UPDATE_SQL,
            LOCK_FOR_DELETE_SQL,
            DELETE_SQL,
        ] {
            assert!(sql.contains("tenant_id = $1"), "unscoped SQL: {sql}");
        }
        assert!(CREATE_SQL.contains("id, tenant_id, name"));
    }

    #[test]
    fn list_queries_lock_the_binary_ordering_contract() {
        assert!(LIST_SQL.contains(r#"ORDER BY name COLLATE "C" ASC, id COLLATE "C" ASC"#));
        assert!(LIST_FOR_RULE_SNAPSHOT_SQL.contains(
            r#"ORDER BY tenant_id COLLATE "C" ASC,
              name COLLATE "C" ASC,
              id COLLATE "C" ASC"#
        ));
    }

    #[test]
    fn returned_column_contract_stays_complete() {
        for column in ZONE_COLUMNS.split(", ") {
            assert!(GET_SQL.contains(column));
            assert!(CREATE_SQL.contains(column));
            assert!(UPDATE_SQL.contains(column));
        }
    }
}

pub(crate) fn list_snapshot_on_connection(
    connection: &mut diesel::PgConnection,
) -> Result<Vec<Zone>, PersistenceError> {
    sql_query(LIST_FOR_RULE_SNAPSHOT_SQL)
        .load::<ZoneRow>(connection)
        .map_err(map_diesel_error)?
        .into_iter()
        .map(ZoneRow::into_domain)
        .collect()
}
