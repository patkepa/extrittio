use async_trait::async_trait;
use chrono::{DateTime, Utc};
use extrittio_backend_core::{
    ConstraintName, DeleteZoneOutcome, NewZone, PersistenceError, RuleZoneSnapshotRepository,
    TenantId, Zone, ZonePatch, ZoneRepository,
};
use turso::{Connection, Row, params};

use crate::error::map_error;
use crate::{TursoConnectionHandles, TursoDatabase};

const SELECT_ZONE: &str = "SELECT id, tenant_id, name, description, geometry_type, geometry_json, color, created_at, updated_at FROM zones";

/// Turso implementation of the tenant zone and system rule-snapshot ports.
#[derive(Clone)]
pub struct TursoZoneRepository {
    handles: TursoConnectionHandles,
}

impl TursoZoneRepository {
    #[must_use]
    pub fn new(database: TursoDatabase) -> Self {
        Self::from_handles(database.shared_handles())
    }

    #[must_use]
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }

    /// Construct the migrated repository from the host's existing engine and
    /// writer. This bridge is removed once all Turso repositories own the new
    /// database foundation.
    #[must_use]
    pub fn from_shared_handles(
        database: std::sync::Arc<turso::Database>,
        writer: std::sync::Arc<tokio::sync::Mutex<turso::Connection>>,
    ) -> Self {
        Self::from_handles(TursoConnectionHandles::new(database, writer))
    }

    #[must_use]
    pub fn shared_handles(&self) -> TursoConnectionHandles {
        self.handles.clone()
    }
}

#[async_trait]
impl ZoneRepository for TursoZoneRepository {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<Zone>, PersistenceError> {
        let connection = self.handles.connect()?;
        list_from(
            &connection,
            &format!(
                "{SELECT_ZONE} WHERE tenant_id = ?1 \
                 ORDER BY name COLLATE BINARY ASC, id COLLATE BINARY ASC"
            ),
            params![tenant.as_str()],
        )
        .await
    }

    async fn get(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<Option<Zone>, PersistenceError> {
        get_from(&self.handles.connect()?, tenant, zone_id).await
    }

    async fn create(&self, tenant: &TenantId, zone: NewZone) -> Result<Zone, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        if get_from(&writer, tenant, &zone.id).await?.is_some() {
            return Err(PersistenceError::UniqueViolation {
                constraint: ConstraintName::new("zones.id"),
            });
        }
        if name_exists(&writer, tenant, &zone.name, None).await? {
            return Err(PersistenceError::UniqueViolation {
                constraint: ConstraintName::new("zones.tenant_name"),
            });
        }
        let now = Utc::now().timestamp_micros();
        let geometry = serde_json::to_string(&zone.geometry_json)
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        writer
            .execute(
                "INSERT INTO zones (
                    id, tenant_id, name, description, geometry_type, geometry_json,
                    color, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    zone.id.clone(),
                    tenant.as_str(),
                    zone.name,
                    zone.description,
                    zone.geometry_type,
                    geometry,
                    zone.color,
                    now
                ],
            )
            .await
            .map_err(map_error)?;
        get_from(&writer, tenant, &zone.id)
            .await?
            .ok_or(PersistenceError::NotFound)
    }

    async fn update(
        &self,
        tenant: &TenantId,
        zone_id: &str,
        patch: ZonePatch,
    ) -> Result<Option<Zone>, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        if let Some(name) = patch.name.as_deref()
            && name_exists(&transaction, tenant, name, Some(zone_id)).await?
        {
            transaction.rollback().await.map_err(map_error)?;
            return Err(PersistenceError::UniqueViolation {
                constraint: ConstraintName::new("zones.tenant_name"),
            });
        }
        let affected = transaction
            .execute(
                "UPDATE zones SET updated_at = ?3 WHERE tenant_id = ?1 AND id = ?2",
                params![
                    tenant.as_str(),
                    zone_id,
                    patch.updated_at.timestamp_micros()
                ],
            )
            .await
            .map_err(map_error)?;
        if affected == 0 {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(None);
        }

        if let Some(value) = patch.name {
            transaction
                .execute(
                    "UPDATE zones SET name = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), zone_id, value],
                )
                .await
                .map_err(map_error)?;
        }
        if let Some(value) = patch.description {
            transaction
                .execute(
                    "UPDATE zones SET description = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), zone_id, value],
                )
                .await
                .map_err(map_error)?;
        }
        if let Some(value) = patch.geometry_type {
            transaction
                .execute(
                    "UPDATE zones SET geometry_type = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), zone_id, value],
                )
                .await
                .map_err(map_error)?;
        }
        if let Some(value) = patch.geometry_json {
            let value = serde_json::to_string(&value)
                .map_err(|error| PersistenceError::Internal(error.to_string()))?;
            transaction
                .execute(
                    "UPDATE zones SET geometry_json = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), zone_id, value],
                )
                .await
                .map_err(map_error)?;
        }
        if let Some(value) = patch.color {
            transaction
                .execute(
                    "UPDATE zones SET color = ?3 WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), zone_id, value],
                )
                .await
                .map_err(map_error)?;
        }

        let updated = get_from(&transaction, tenant, zone_id).await?;
        transaction.commit().await.map_err(map_error)?;
        Ok(updated)
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<DeleteZoneOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        if get_from(&transaction, tenant, zone_id).await?.is_none() {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(DeleteZoneOutcome::NotFound);
        }

        let mut rows = transaction
            .query(
                "SELECT count(*) FROM rule_conditions WHERE tenant_id = ?1 AND zone_id = ?2",
                params![tenant.as_str(), zone_id],
            )
            .await
            .map_err(map_error)?;
        let references = rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(map_error)?;
        drop(rows);

        if references > 0 {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(DeleteZoneOutcome::InUse);
        }

        transaction
            .execute(
                "DELETE FROM zones WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), zone_id],
            )
            .await
            .map_err(map_error)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(DeleteZoneOutcome::Deleted)
    }
}

#[async_trait]
impl RuleZoneSnapshotRepository for TursoZoneRepository {
    async fn list_for_rule_snapshot(&self) -> Result<Vec<Zone>, PersistenceError> {
        let connection = self.handles.connect()?;
        list_from(
            &connection,
            &format!(
                "{SELECT_ZONE} ORDER BY \
                 tenant_id COLLATE BINARY ASC, \
                 name COLLATE BINARY ASC, \
                 id COLLATE BINARY ASC"
            ),
            (),
        )
        .await
    }
}

async fn get_from(
    connection: &Connection,
    tenant: &TenantId,
    zone_id: &str,
) -> Result<Option<Zone>, PersistenceError> {
    let mut rows = connection
        .query(
            &format!("{SELECT_ZONE} WHERE tenant_id = ?1 AND id = ?2"),
            params![tenant.as_str(), zone_id],
        )
        .await
        .map_err(map_error)?;
    rows.next()
        .await
        .map_err(map_error)?
        .map(|row| decode(&row))
        .transpose()
}

async fn list_from(
    connection: &Connection,
    sql: &str,
    params: impl turso::IntoParams,
) -> Result<Vec<Zone>, PersistenceError> {
    let mut rows = connection.query(sql, params).await.map_err(map_error)?;
    let mut zones = Vec::new();
    while let Some(row) = rows.next().await.map_err(map_error)? {
        zones.push(decode(&row)?);
    }
    Ok(zones)
}

async fn name_exists(
    connection: &Connection,
    tenant: &TenantId,
    name: &str,
    except_id: Option<&str>,
) -> Result<bool, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT EXISTS(
                SELECT 1 FROM zones
                 WHERE tenant_id = ?1 AND name = ?2
                   AND (?3 IS NULL OR id <> ?3)
             )",
            params![tenant.as_str(), name, except_id],
        )
        .await
        .map_err(map_error)?;
    rows.next()
        .await
        .map_err(map_error)?
        .ok_or(PersistenceError::NotFound)?
        .get::<i64>(0)
        .map(|present| present != 0)
        .map_err(map_error)
}

fn decode(row: &Row) -> Result<Zone, PersistenceError> {
    let tenant_id: String = row.get(1).map_err(map_error)?;
    let geometry_json: String = row.get(5).map_err(map_error)?;
    Ok(Zone {
        id: row.get(0).map_err(map_error)?,
        tenant_id: TenantId::new(tenant_id)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        name: row.get(2).map_err(map_error)?,
        description: row.get(3).map_err(map_error)?,
        geometry_type: row.get(4).map_err(map_error)?,
        geometry_json: serde_json::from_str(&geometry_json)
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
        color: row.get(6).map_err(map_error)?,
        created_at: decode_datetime(row.get(7).map_err(map_error)?)?,
        updated_at: decode_datetime(row.get(8).map_err(map_error)?)?,
    })
}

fn decode_datetime(micros: i64) -> Result<DateTime<Utc>, PersistenceError> {
    DateTime::from_timestamp_micros(micros).ok_or_else(|| {
        PersistenceError::CorruptData(format!("invalid UTC timestamp in database: {micros}"))
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::{TimeZone, Utc};
    use extrittio_backend_core::{
        DeleteZoneOutcome, NewZone, PersistenceError, RuleZoneSnapshotRepository, TenantId,
        ZonePatch, ZoneRepository,
    };
    use serde_json::json;
    use turso::params;

    use super::TursoZoneRepository;
    use crate::{LATEST_SCHEMA_VERSION, TursoDatabase};

    fn new_zone(id: &str, name: &str) -> NewZone {
        NewZone {
            id: id.into(),
            name: name.into(),
            description: format!("{name} description"),
            geometry_type: "circle".into(),
            geometry_json: json!({"center": [52.23, 21.01], "radius_meters": 50.0}),
            color: "#123456".into(),
        }
    }

    #[tokio::test]
    async fn fresh_database_satisfies_the_zone_contract() {
        let directory = tempfile::tempdir().unwrap();
        let database = TursoDatabase::open_and_migrate(
            directory.path(),
            &directory.path().join("extrittio.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        database.migrate().await.unwrap();
        assert_eq!(
            database.schema_version().await.unwrap(),
            LATEST_SCHEMA_VERSION
        );
        database.health().await.unwrap();
        database.integrity_check().await.unwrap();

        let connection = database.shared_handles().connect().unwrap();
        connection
            .execute(
                "INSERT INTO organizations (id, name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)",
                params!["tenant-b", "Tenant B", 1_i64],
            )
            .await
            .unwrap();

        let repository = TursoZoneRepository::new(database.clone());
        let default_tenant = TenantId::new("default").unwrap();
        let tenant_b = TenantId::new("tenant-b").unwrap();

        repository
            .create(&default_tenant, new_zone("zone-z", "zebra"))
            .await
            .unwrap();
        let alpha = repository
            .create(&default_tenant, new_zone("zone-a", "Alpha"))
            .await
            .unwrap();
        repository
            .create(&default_tenant, new_zone("zone-l", "alpha"))
            .await
            .unwrap();
        repository
            .create(&tenant_b, new_zone("zone-b", "Beta"))
            .await
            .unwrap();
        repository
            .create(&tenant_b, new_zone("zone-b2", "Alpha"))
            .await
            .unwrap();

        assert_eq!(
            repository
                .list(&default_tenant)
                .await
                .unwrap()
                .into_iter()
                .map(|zone| zone.id)
                .collect::<Vec<_>>(),
            ["zone-a", "zone-l", "zone-z"]
        );
        assert_eq!(
            repository
                .list(&tenant_b)
                .await
                .unwrap()
                .into_iter()
                .map(|zone| zone.id)
                .collect::<Vec<_>>(),
            ["zone-b2", "zone-b"]
        );
        assert!(
            repository
                .get(&tenant_b, &alpha.id)
                .await
                .unwrap()
                .is_none()
        );

        let duplicate = repository
            .create(&default_tenant, new_zone("zone-duplicate", "Alpha"))
            .await
            .unwrap_err();
        assert!(
            matches!(
                duplicate,
                PersistenceError::UniqueViolation { ref constraint }
                    if constraint.as_str() == "zones.tenant_name"
            ),
            "unexpected duplicate-name result: {duplicate:?}"
        );

        let updated_at = Utc.timestamp_micros(1_700_000_000_123_456).unwrap();
        let updated = repository
            .update(
                &default_tenant,
                &alpha.id,
                ZonePatch {
                    name: Some("Aardvark".into()),
                    description: None,
                    geometry_type: None,
                    geometry_json: None,
                    color: Some("#abcdef".into()),
                    updated_at,
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "Aardvark");
        assert_eq!(updated.description, "Alpha description");
        assert_eq!(updated.color, "#abcdef");
        assert_eq!(updated.updated_at, updated_at);
        assert!(
            repository
                .update(
                    &tenant_b,
                    &alpha.id,
                    ZonePatch {
                        name: Some("invisible".into()),
                        description: None,
                        geometry_type: None,
                        geometry_json: None,
                        color: None,
                        updated_at,
                    },
                )
                .await
                .unwrap()
                .is_none()
        );

        let snapshot = repository.list_for_rule_snapshot().await.unwrap();
        assert_eq!(
            snapshot
                .into_iter()
                .map(|zone| (zone.tenant_id.to_string(), zone.id))
                .collect::<Vec<_>>(),
            [
                ("default".into(), "zone-a".into()),
                ("default".into(), "zone-l".into()),
                ("default".into(), "zone-z".into()),
                ("tenant-b".into(), "zone-b2".into()),
                ("tenant-b".into(), "zone-b".into()),
            ]
        );

        connection
            .execute(
                "INSERT INTO rules (
                    id, tenant_id, name, description, enabled, trigger_type,
                    target_type, target_id, cooldown_seconds, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, NULL, 1, 'telemetry', 'device', NULL, 0, ?4, ?4)",
                params!["rule-1", "default", "Rule", 2_i64],
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO rule_conditions (
                    id, tenant_id, rule_id, field, operator, value, condition_group, zone_id
                 ) VALUES (?1, ?2, ?3, 'temperature', 'greater_than', '10', 0, ?4)",
                params!["condition-1", "default", "rule-1", alpha.id.clone()],
            )
            .await
            .unwrap();
        assert_eq!(
            repository.delete(&default_tenant, &alpha.id).await.unwrap(),
            DeleteZoneOutcome::InUse
        );

        connection
            .execute(
                "DELETE FROM rule_conditions WHERE id = ?1",
                params!["condition-1"],
            )
            .await
            .unwrap();
        assert_eq!(
            repository.delete(&default_tenant, &alpha.id).await.unwrap(),
            DeleteZoneOutcome::Deleted
        );
        assert_eq!(
            repository.delete(&default_tenant, &alpha.id).await.unwrap(),
            DeleteZoneOutcome::NotFound
        );

        let handles = database.shared_handles();
        let clone = TursoZoneRepository::from_shared_handles(handles.database(), handles.writer());
        assert!(
            clone
                .get(&default_tenant, "zone-z")
                .await
                .unwrap()
                .is_some()
        );
    }
}
