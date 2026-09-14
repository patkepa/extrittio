use crate::error::map_diesel_error;
use crate::models::{
    Device, DeviceCertificate, DeviceType, Fleet, NewDevice, NewDeviceCertificate, NewDeviceShadow,
    UpdateDevice,
};
use crate::schema::{device_certificates, device_shadows, device_types, devices, fleets};
use crate::{PostgresExecutor, PostgresPool};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::sql_types::{Jsonb, Nullable, Text, Timestamptz};
use diesel::{Connection, PgTextExpressionMethods, prelude::*};
use extrittio_backend_core::certificates::NewDeviceCertificateRecord;
use extrittio_backend_core::device_types::DeviceTypeRecord;
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
type JoinedDevice = (Device, DeviceType, Option<Fleet>);

type BoxedDeviceQuery<'a> = diesel::dsl::IntoBoxed<
    'a,
    diesel::dsl::LeftJoin<
        diesel::dsl::InnerJoin<devices::table, device_types::table>,
        fleets::table,
    >,
    diesel::pg::Pg,
>;

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

fn filtered_query<'a>(tenant_id: &'a str, filter: &'a DeviceFilter) -> BoxedDeviceQuery<'a> {
    let mut query = devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .filter(devices::tenant_id.eq(tenant_id))
        .into_boxed();
    if let Some(status) = &filter.status {
        query = query.filter(devices::status.eq(status));
    }
    if let Some(search) = &filter.search {
        let pattern = format!("%{search}%");
        query = query.filter(
            devices::name
                .ilike(pattern.clone())
                .or(device_types::name.ilike(pattern)),
        );
    }
    if let Some(fleet_id) = filter.fleet_id {
        query = query.filter(devices::fleet_id.eq(fleet_id));
    }
    query
}

fn joined_for_device(
    connection: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
) -> QueryResult<Option<JoinedDevice>> {
    devices::table
        .inner_join(device_types::table)
        .left_join(fleets::table)
        .filter(devices::tenant_id.eq(tenant_id))
        .filter(devices::id.eq(device_id))
        .select((
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .first(connection)
        .optional()
}

fn to_details((device, device_type, fleet): JoinedDevice) -> DeviceDetails {
    DeviceDetails {
        device: DeviceRecord {
            id: device.id,
            name: device.name,
            device_type_id: device.device_type_id,
            fleet_id: device.fleet_id,
            status: device.status,
            firmware: device.firmware,
            last_seen: device.last_seen.map(|value| value.and_utc()),
            uptime_seconds: device.uptime_seconds,
            latest_latitude: device.latest_latitude,
            latest_longitude: device.latest_longitude,
            declared_connections: device.declared_connections,
        },
        device_type: DeviceTypeRecord {
            id: device_type.id,
            name: device_type.name,
            icon: device_type.icon,
            color_hex: device_type.color_hex,
        },
        fleet: fleet.map(|fleet| FleetRecord {
            id: fleet.id,
            name: fleet.name,
        }),
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
                let filter = DeviceFilter {
                    status: query.status,
                    search: query.search,
                    fleet_id: query.fleet_id,
                };
                let total = filtered_query(&tenant_id, &filter)
                    .count()
                    .get_result::<i64>(connection)
                    .map_err(map_diesel_error)?;
                let rows = filtered_query(&tenant_id, &filter)
                    .select((
                        Device::as_select(),
                        DeviceType::as_select(),
                        Option::<Fleet>::as_select(),
                    ))
                    .order((devices::name.asc(), devices::id.asc()))
                    .limit(query.limit)
                    .offset(query.offset)
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
                                device_type_id: record.device_type_id,
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
                            device_type_id: record.device_type_id,
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
                filtered_query(&tenant_id, &filter)
                    .select(devices::id)
                    .order((devices::name.asc(), devices::id.asc()))
                    .load(connection)
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
