use crate::error::map_diesel_error;
use crate::models::{
    DeviceCertificate, NewDevice, NewDeviceCertificate, NewDeviceShadow, UpdateDevice,
};
use crate::schema::{device_certificates, device_shadows, devices};
use crate::{PostgresExecutor, PostgresPool};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::sql_types::{BigInt, Integer, Jsonb, Nullable, Text, Timestamptz};
use diesel::{Connection, prelude::*};
use extrittio_backend_core::certificates::NewDeviceCertificateRecord;
use extrittio_backend_core::devices::*;
use extrittio_backend_core::fleets::FleetRecord;
use extrittio_backend_core::{PersistenceError, TenantId};
use serde_json::Value;
#[derive(Clone)]
pub struct PostgresDeviceRepository {
    executor: PostgresExecutor,
}
impl PostgresDeviceRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
#[derive(QueryableByName)]
struct JoinedDevice {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Integer>)]
    fleet_id: Option<i32>,
    #[diesel(sql_type = Nullable<Text>)]
    fleet_name: Option<String>,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Text)]
    firmware: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    last_seen: Option<DateTime<Utc>>,
    #[diesel(sql_type = Integer)]
    uptime_seconds: i32,
    #[diesel(sql_type = Jsonb)]
    declared_connections: Value,
    #[diesel(sql_type = Text)]
    blueprint_id: String,
    #[diesel(sql_type = Text)]
    revision_id: String,
    #[diesel(sql_type = Text)]
    blueprint_key: String,
    #[diesel(sql_type = Text)]
    blueprint_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    blueprint_icon: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    blueprint_color: Option<String>,
}
#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}
#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = Text)]
    id: String,
}

const DEVICE_JOINS: &str = "FROM devices d
    JOIN device_contract_assignments a ON a.tenant_id=d.tenant_id AND a.device_id=d.id
    JOIN device_contracts c ON c.tenant_id=a.tenant_id AND c.device_id=a.device_id AND c.id=a.desired_contract_id
    JOIN device_blueprint_revisions r ON r.tenant_id=c.tenant_id AND r.id=c.blueprint_revision_id
    JOIN device_blueprints b ON b.tenant_id=r.tenant_id AND b.id=r.blueprint_id
    LEFT JOIN fleets f ON f.tenant_id=d.tenant_id AND f.id=d.fleet_id";
const DEVICE_FILTER: &str = "d.tenant_id=$1 AND ($2::text IS NULL OR d.status=$2)
    AND ($3::text IS NULL OR d.name ILIKE $3 OR b.name ILIKE $3)
    AND ($4::integer IS NULL OR d.fleet_id=$4)";
fn details_sql() -> String {
    format!(
        "SELECT d.id,d.name,d.fleet_id,f.name AS fleet_name,d.status,d.firmware,
        d.last_seen,d.uptime_seconds,d.declared_connections,
        b.id AS blueprint_id,r.id AS revision_id,b.blueprint_key,b.name AS blueprint_name,
        r.document->'metadata'->>'icon' AS blueprint_icon,
        r.document->'metadata'->>'color' AS blueprint_color {DEVICE_JOINS}"
    )
}

#[derive(QueryableByName)]
struct DeviceContractRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    device_id: String,
    #[diesel(sql_type = Text)]
    blueprint_revision_id: String,
    #[diesel(sql_type = Jsonb)]
    document: Value,
    #[diesel(sql_type = Text)]
    contract_hash: String,
    #[diesel(sql_type = Text)]
    assignment_status: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    acknowledged_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = Nullable<Text>)]
    error: Option<String>,
    #[diesel(sql_type = Timestamptz)]
    created_at: DateTime<Utc>,
}

impl From<DeviceContractRow> for DeviceContractRecord {
    fn from(row: DeviceContractRow) -> Self {
        Self {
            id: row.id,
            device_id: row.device_id,
            blueprint_revision_id: row.blueprint_revision_id,
            document: row.document,
            contract_hash: row.contract_hash,
            assignment_status: row.assignment_status,
            acknowledged_at: row.acknowledged_at,
            error: row.error,
            created_at: row.created_at,
        }
    }
}

fn joined_for_device(
    connection: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
) -> QueryResult<Option<JoinedDevice>> {
    diesel::sql_query(format!(
        "{} WHERE d.tenant_id=$1 AND d.id=$2",
        details_sql()
    ))
    .bind::<Text, _>(tenant_id)
    .bind::<Text, _>(device_id)
    .get_result(connection)
    .optional()
}
fn to_details(row: JoinedDevice) -> DeviceDetails {
    DeviceDetails {
        device: DeviceRecord {
            id: row.id,
            name: row.name,
            fleet_id: row.fleet_id,
            status: row.status,
            firmware: row.firmware,
            last_seen: row.last_seen,
            uptime_seconds: row.uptime_seconds,
            declared_connections: row.declared_connections,
        },
        blueprint: DeviceBlueprintIdentity {
            id: row.blueprint_id,
            revision_id: row.revision_id,
            key: row.blueprint_key,
            name: row.blueprint_name,
            icon: row.blueprint_icon,
            color: row.blueprint_color,
        },
        fleet: row
            .fleet_id
            .zip(row.fleet_name)
            .map(|(id, name)| FleetRecord { id, name }),
    }
}

#[async_trait]
impl DeviceRepository for PostgresDeviceRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        query: DeviceListQuery,
    ) -> Result<DeviceList, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let search = query.search.map(|value| format!("%{value}%"));
                let total = diesel::sql_query(format!(
                    "SELECT count(*) AS count {DEVICE_JOINS} WHERE {DEVICE_FILTER}"
                ))
                .bind::<Text, _>(&tenant_id)
                .bind::<Nullable<Text>, _>(&query.status)
                .bind::<Nullable<Text>, _>(&search)
                .bind::<Nullable<Integer>, _>(query.fleet_id)
                .get_result::<CountRow>(connection)
                .map_err(map_diesel_error)?
                .count;
                let rows = diesel::sql_query(format!(
                    "{} WHERE {DEVICE_FILTER} ORDER BY d.name,d.id LIMIT $5 OFFSET $6",
                    details_sql()
                ))
                .bind::<Text, _>(&tenant_id)
                .bind::<Nullable<Text>, _>(&query.status)
                .bind::<Nullable<Text>, _>(&search)
                .bind::<Nullable<Integer>, _>(query.fleet_id)
                .bind::<BigInt, _>(query.limit)
                .bind::<BigInt, _>(query.offset)
                .load::<JoinedDevice>(connection)
                .map_err(map_diesel_error)?;
                Ok(DeviceList {
                    records: rows.into_iter().map(to_details).collect(),
                    total,
                })
            })
            .await
    }

    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                let Some(row) = joined_for_device(connection, &tenant_id, &device_id)
                    .map_err(map_diesel_error)?
                else {
                    return Ok(None);
                };
                Ok(Some(to_details(row)))
            })
            .await
    }

    async fn assigned_contract(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<DeviceContractRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                diesel::sql_query(
                    "SELECT c.id, c.device_id, c.blueprint_revision_id, c.document,
                            c.contract_hash, a.status AS assignment_status,
                            a.acknowledged_at, a.error, c.created_at
                     FROM device_contract_assignments a
                     JOIN device_contracts c
                       ON c.tenant_id = a.tenant_id AND c.id = a.desired_contract_id
                     WHERE a.tenant_id = $1 AND a.device_id = $2",
                )
                .bind::<Text, _>(tenant_id)
                .bind::<Text, _>(device_id)
                .get_result::<DeviceContractRow>(connection)
                .optional()
                .map(|record| record.map(Into::into))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceRecord,
        certificate: Option<NewDeviceCertificateRecord>,
    ) -> Result<DeviceDetails, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let device_id = record.id.clone();
                let contract = record.contract;
                let initial_configuration = contract.initial_configuration;
                let row = connection
                    .transaction(|connection| {
                        diesel::insert_into(devices::table)
                            .values(NewDevice {
                                id: record.id,
                                tenant_id: tenant_id.clone(),
                                name: record.name,
                                fleet_id: record.fleet_id,
                                firmware: record.firmware,
                            })
                            .execute(connection)?;
                        diesel::insert_into(device_shadows::table)
                            .values(NewDeviceShadow {
                                device_id: device_id.clone(),
                                tenant_id: tenant_id.clone(),
                            })
                            .execute(connection)?;
                        if let Some(certificate) = certificate {
                            diesel::insert_into(device_certificates::table)
                                .values(NewDeviceCertificate {
                                    tenant_id: tenant_id.clone(),
                                    device_id: device_id.clone(),
                                    private_key_pem: certificate.private_key_pem,
                                    certificate_pem: certificate.certificate_pem,
                                    fingerprint: certificate.fingerprint,
                                    expires_at: certificate.expires_at.naive_utc(),
                                })
                                .returning(DeviceCertificate::as_returning())
                                .get_result::<DeviceCertificate>(connection)?;
                        }
                        {
                            diesel::sql_query(
                                "INSERT INTO device_contracts
                                    (id, tenant_id, device_id, blueprint_revision_id, document,
                                     contract_hash, created_at)
                                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
                            )
                            .bind::<Text, _>(&contract.id)
                            .bind::<Text, _>(&tenant_id)
                            .bind::<Text, _>(&device_id)
                            .bind::<Text, _>(&contract.blueprint_revision_id)
                            .bind::<Jsonb, _>(&contract.document)
                            .bind::<Text, _>(&contract.contract_hash)
                            .bind::<Timestamptz, _>(contract.created_at)
                            .execute(connection)?;
                            diesel::sql_query(
                                "INSERT INTO device_contract_assignments
                                    (tenant_id, device_id, desired_contract_id, status,
                                     created_at, updated_at)
                                 VALUES ($1, $2, $3, 'pending', $4, $4)",
                            )
                            .bind::<Text, _>(&tenant_id)
                            .bind::<Text, _>(&device_id)
                            .bind::<Text, _>(&contract.id)
                            .bind::<Timestamptz, _>(contract.created_at)
                            .execute(connection)?;
                        }
                        if let Some(configuration) = initial_configuration {
                            diesel::insert_into(crate::schema::device_configs::table)
                                .values(crate::models::NewDeviceConfig {
                                    device_id: device_id.clone(),
                                    tenant_id: tenant_id.clone(),
                                    config: configuration,
                                })
                                .execute(connection)?;
                        }
                        joined_for_device(connection, &tenant_id, &device_id)?
                            .ok_or(diesel::result::Error::NotFound)
                    })
                    .map_err(map_diesel_error)?;
                Ok(to_details(row))
            })
            .await
    }

    async fn update(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: UpdateDeviceRecord,
    ) -> Result<Option<DeviceDetails>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let rows = diesel::update(
                            devices::table
                                .filter(devices::tenant_id.eq(&tenant_id))
                                .filter(devices::id.eq(&device_id)),
                        )
                        .set(UpdateDevice {
                            name: record.name,
                            fleet_id: record.fleet_id,
                            firmware: record.firmware,
                            updated_at: record.updated_at.map(|value| value.naive_utc()),
                            ..Default::default()
                        })
                        .execute(connection)?;
                        if rows == 0 {
                            return Ok(None);
                        }
                        joined_for_device(connection, &tenant_id, &device_id)
                    })
                    .map_err(map_diesel_error)
                    .map(|row| row.map(to_details))
            })
            .await
    }

    async fn delete(&self, tenant: &TenantId, device_id: &str) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                diesel::delete(
                    devices::table
                        .filter(devices::tenant_id.eq(tenant_id))
                        .filter(devices::id.eq(device_id)),
                )
                .execute(connection)
                .map(|rows| rows == 1)
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn resolve_ids(
        &self,
        tenant: &TenantId,
        filter: DeviceFilter,
    ) -> Result<Vec<String>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let search = filter.search.map(|value| format!("%{value}%"));
                diesel::sql_query(format!(
                    "SELECT d.id {DEVICE_JOINS} WHERE {DEVICE_FILTER} ORDER BY d.name,d.id"
                ))
                .bind::<Text, _>(&tenant_id)
                .bind::<Nullable<Text>, _>(&filter.status)
                .bind::<Nullable<Text>, _>(&search)
                .bind::<Nullable<Integer>, _>(filter.fleet_id)
                .load::<IdRow>(connection)
                .map(|rows| rows.into_iter().map(|row| row.id).collect())
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn bulk_assign_fleet(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
        fleet_id: Option<i32>,
        updated_at: DateTime<Utc>,
    ) -> Result<usize, PersistenceError> {
        if device_ids.is_empty() {
            return Ok(0);
        }
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::update(
                    devices::table
                        .filter(devices::tenant_id.eq(tenant_id))
                        .filter(devices::id.eq_any(device_ids)),
                )
                .set((
                    devices::fleet_id.eq(fleet_id),
                    devices::updated_at.eq(updated_at.naive_utc()),
                ))
                .execute(connection)
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn bulk_delete(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
    ) -> Result<usize, PersistenceError> {
        if device_ids.is_empty() {
            return Ok(0);
        }
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::delete(
                    devices::table
                        .filter(devices::tenant_id.eq(tenant_id))
                        .filter(devices::id.eq_any(device_ids)),
                )
                .execute(connection)
                .map_err(map_diesel_error)
            })
            .await
    }
}
