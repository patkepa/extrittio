use std::collections::HashMap;

use async_trait::async_trait;
use diesel::prelude::*;

use crate::models::{Fleet, NewFleet};
use crate::schema::{devices, fleets};
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::fleets::FleetRepository;
use extrittio_backend_core::fleets::{CreateFleetRecord, FleetList, FleetRecord, FleetSummary};

use crate::error::map_diesel_error;
use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresFleetRepository {
    executor: PostgresExecutor,
}
impl PostgresFleetRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}

fn to_record(row: Fleet) -> FleetRecord {
    FleetRecord {
        id: row.id,
        name: row.name,
    }
}

#[async_trait]
impl FleetRepository for PostgresFleetRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<FleetList, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let total = fleets::table
                    .filter(fleets::tenant_id.eq(&tenant_id))
                    .count()
                    .get_result(connection)
                    .map_err(map_diesel_error)?;
                let rows = fleets::table
                    .filter(fleets::tenant_id.eq(&tenant_id))
                    .select(Fleet::as_select())
                    .order((
                        diesel::dsl::sql::<diesel::sql_types::Text>("name COLLATE \"C\"").asc(),
                        fleets::id.asc(),
                    ))
                    .limit(limit)
                    .offset(offset)
                    .load::<Fleet>(connection)
                    .map_err(map_diesel_error)?;
                let counts = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .group_by(devices::fleet_id)
                    .select((devices::fleet_id, diesel::dsl::count(devices::id)))
                    .load::<(Option<i32>, i64)>(connection)
                    .map_err(map_diesel_error)?;
                let counts: HashMap<i32, i64> = counts
                    .into_iter()
                    .filter_map(|(fleet_id, count)| fleet_id.map(|id| (id, count)))
                    .collect();
                let records = rows
                    .into_iter()
                    .map(|row| {
                        let device_count = counts.get(&row.id).copied().unwrap_or(0);
                        FleetSummary {
                            fleet: to_record(row),
                            device_count,
                        }
                    })
                    .collect();

                Ok(FleetList { records, total })
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateFleetRecord,
    ) -> Result<FleetRecord, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::insert_into(fleets::table)
                    .values(NewFleet {
                        tenant_id,
                        name: record.name,
                    })
                    .returning(Fleet::as_returning())
                    .get_result(connection)
                    .map(to_record)
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn rename(
        &self,
        tenant: &TenantId,
        id: i32,
        name: String,
    ) -> Result<Option<FleetRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::update(
                    fleets::table
                        .filter(fleets::tenant_id.eq(tenant_id))
                        .filter(fleets::id.eq(id)),
                )
                .set(fleets::name.eq(name))
                .returning(Fleet::as_returning())
                .get_result(connection)
                .optional()
                .map(|row| row.map(to_record))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::delete(
                    fleets::table
                        .filter(fleets::tenant_id.eq(tenant_id))
                        .filter(fleets::id.eq(id)),
                )
                .execute(connection)
                .map(|rows| rows > 0)
                .map_err(map_diesel_error)
            })
            .await
    }
}
