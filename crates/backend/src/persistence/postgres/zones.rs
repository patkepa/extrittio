use async_trait::async_trait;
use diesel::{Connection, OptionalExtension, prelude::*};

use crate::db::models::{NewZone, UpdateZone, Zone};
use crate::db::schema::rule_conditions;
use crate::domains::zones::port::ZoneRepository;
use crate::domains::zones::types::{
    DeleteZoneOutcome, NewZoneRecord, UpdateZoneRecord, ZoneRecord,
};
use crate::persistence::PersistenceError;
use crate::repositories::zone_repo;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

fn zone_record(zone: Zone) -> ZoneRecord {
    ZoneRecord {
        id: zone.id,
        tenant_id: zone.tenant_id,
        name: zone.name,
        description: zone.description,
        geometry_type: zone.geometry_type,
        geometry_json: zone.geometry_json,
        color: zone.color,
        created_at: zone.created_at,
        updated_at: zone.updated_at,
    }
}

#[async_trait]
impl ZoneRepository for PostgresAdapter {
    async fn list(&self, tenant: &TenantId) -> Result<Vec<ZoneRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                zone_repo::list_zones(connection, &tenant_id)
                    .map(|zones| zones.into_iter().map(zone_record).collect())
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn list_all(&self) -> Result<Vec<ZoneRecord>, PersistenceError> {
        self.executor
            .run(move |connection| {
                zone_repo::list_all_zones(connection)
                    .map(|zones| zones.into_iter().map(zone_record).collect())
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn get(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<Option<ZoneRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let zone_id = zone_id.to_string();
        self.executor
            .run(move |connection| {
                zone_repo::get_zone(connection, &tenant_id, &zone_id)
                    .optional()
                    .map(|zone| zone.map(zone_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: NewZoneRecord,
    ) -> Result<ZoneRecord, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                zone_repo::insert_zone(
                    connection,
                    &tenant_id,
                    &NewZone {
                        id: record.id,
                        tenant_id: tenant_id.clone(),
                        name: record.name,
                        description: record.description,
                        geometry_type: record.geometry_type,
                        geometry_json: record.geometry_json,
                        color: record.color,
                    },
                )
                .map(zone_record)
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn update(
        &self,
        tenant: &TenantId,
        zone_id: &str,
        record: UpdateZoneRecord,
    ) -> Result<Option<ZoneRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let zone_id = zone_id.to_string();
        self.executor
            .run(move |connection| {
                zone_repo::update_zone(
                    connection,
                    &tenant_id,
                    &zone_id,
                    &UpdateZone {
                        name: record.name,
                        description: record.description,
                        geometry_type: record.geometry_type,
                        geometry_json: record.geometry_json,
                        color: record.color,
                        updated_at: Some(record.updated_at),
                    },
                )
                .optional()
                .map(|zone| zone.map(zone_record))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete(
        &self,
        tenant: &TenantId,
        zone_id: &str,
    ) -> Result<DeleteZoneOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let zone_id = zone_id.to_string();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        if zone_repo::get_zone(connection, &tenant_id, &zone_id)
                            .optional()?
                            .is_none()
                        {
                            return Ok(DeleteZoneOutcome::NotFound);
                        }
                        let references: i64 = rule_conditions::table
                            .filter(rule_conditions::tenant_id.eq(&tenant_id))
                            .filter(rule_conditions::zone_id.eq(&zone_id))
                            .count()
                            .get_result(connection)?;
                        if references > 0 {
                            return Ok(DeleteZoneOutcome::InUse);
                        }
                        zone_repo::delete_zone(connection, &tenant_id, &zone_id)?;
                        Ok(DeleteZoneOutcome::Deleted)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }
}
