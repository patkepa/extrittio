use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::Connection;
use diesel::PgTextExpressionMethods;
use diesel::prelude::*;
use diesel::sql_types::{Jsonb, Nullable, Text, Timestamptz};
use serde_json::Value;

use crate::db::models::{
    Device, DeviceCertificate, DeviceType, Fleet, NewDevice, NewDeviceCertificate, NewDeviceLog,
    NewDeviceShadow, UpdateDevice,
};
use crate::db::schema::{
    device_certificates, device_logs, device_shadows, device_types, devices, fleets,
};
use crate::domains::device_types::types::DeviceTypeRecord;
use crate::domains::devices::repository::DeviceRepository;
use crate::domains::devices::types::{
    CreateDeviceRecord, DeviceContractRecord, DeviceDetails, DeviceFilter, DeviceIngressContext,
    DeviceList, DeviceListQuery, DeviceRecord, DeviceWriteOutcome, HeartbeatWrite,
    OfflineTransition, OfflineWriteOutcome, UpdateDeviceRecord,
};
use crate::domains::fleets::types::FleetRecord;
use crate::domains::identity::certificate_types::NewDeviceCertificateRecord;
use crate::error::AppError;
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::PostgresAdapter;
use super::executor::map_diesel_error;
use super::outbox::enqueue_pending_actions;

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

#[derive(QueryableByName)]
struct BlueprintIdRow {
    #[diesel(sql_type = Text)]
    blueprint_id: String,
}

fn blueprint_id_for_device(
    connection: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
) -> QueryResult<Option<String>> {
    diesel::sql_query(
        "SELECT revision.blueprint_id
         FROM device_contract_assignments assignment
         JOIN device_contracts contract
           ON contract.tenant_id = assignment.tenant_id
          AND contract.id = assignment.desired_contract_id
         JOIN device_blueprint_revisions revision
           ON revision.tenant_id = contract.tenant_id
          AND revision.id = contract.blueprint_revision_id
         WHERE assignment.tenant_id = $1 AND assignment.device_id = $2",
    )
    .bind::<Text, _>(tenant_id)
    .bind::<Text, _>(device_id)
    .get_result::<BlueprintIdRow>(connection)
    .optional()
    .map(|row| row.map(|row| row.blueprint_id))
}

fn to_ingress_context(
    device: Device,
    blueprint_id: Option<String>,
) -> Result<DeviceIngressContext, PersistenceError> {
    let identity = DeviceIdentity::new(device.tenant_id, device.id).map_err(|error| {
        PersistenceError::CorruptData(format!("invalid persisted device identity: {error}"))
    })?;
    Ok(DeviceIngressContext {
        identity,
        device_type_id: device.device_type_id,
        fleet_id: device.fleet_id,
        blueprint_id,
        status: device.status,
    })
}

fn map_app_error(error: AppError) -> PersistenceError {
    match error {
        AppError::Database(error) => map_diesel_error(error),
        AppError::Persistence(error) => error,
        other => PersistenceError::Internal(other.to_string()),
    }
}

#[async_trait]
impl DeviceRepository for PostgresAdapter {
    async fn resolve_identity(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceIdentity>, PersistenceError> {
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                let row = devices::table
                    .filter(devices::id.eq(device_id))
                    .select((devices::tenant_id, devices::id))
                    .first::<(String, String)>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                row.map(|(tenant_id, device_id)| {
                    DeviceIdentity::new(tenant_id, device_id).map_err(|error| {
                        PersistenceError::CorruptData(format!(
                            "invalid persisted device identity: {error}"
                        ))
                    })
                })
                .transpose()
            })
            .await
    }

    async fn ingress_context(
        &self,
        identity: &DeviceIdentity,
    ) -> Result<Option<DeviceIngressContext>, PersistenceError> {
        let tenant_id = identity.tenant_id_str().to_owned();
        let device_id = identity.device_id().to_owned();
        self.executor
            .run(move |connection| {
                let device = devices::table
                    .filter(devices::tenant_id.eq(tenant_id))
                    .filter(devices::id.eq(device_id))
                    .select(Device::as_select())
                    .first::<Device>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                let Some(device) = device else {
                    return Ok(None);
                };
                let blueprint_id =
                    blueprint_id_for_device(connection, &device.tenant_id, &device.id)
                        .map_err(map_diesel_error)?;
                to_ingress_context(device, blueprint_id).map(Some)
            })
            .await
    }

    async fn apply_heartbeat(
        &self,
        identity: &DeviceIdentity,
        write: HeartbeatWrite,
    ) -> Result<DeviceWriteOutcome, PersistenceError> {
        let tenant_id = identity.tenant_id_str().to_owned();
        let device_id = identity.device_id().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, AppError, _>(|connection| {
                        let current_status = devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::id.eq(&device_id))
                            .select(devices::status)
                            .for_update()
                            .first::<String>(connection)
                            .optional()?;
                        if current_status.as_deref() != Some(write.expected_status.as_str()) {
                            return Ok(DeviceWriteOutcome {
                                applied: false,
                                actions_enqueued: 0,
                            });
                        }
                        diesel::update(
                            devices::table
                                .filter(devices::tenant_id.eq(&tenant_id))
                                .filter(devices::id.eq(&device_id)),
                        )
                        .set((
                            devices::status.eq(&write.status),
                            devices::firmware.eq(write.firmware),
                            devices::uptime_seconds.eq(write.uptime_seconds),
                            devices::last_seen.eq(write.observed_at),
                            devices::updated_at.eq(write.observed_at),
                        ))
                        .execute(connection)?;
                        if write.expected_status != write.status {
                            diesel::insert_into(device_logs::table)
                                .values(NewDeviceLog {
                                    tenant_id: tenant_id.clone(),
                                    device_id: device_id.clone(),
                                    level: "INFO".to_string(),
                                    message: format!(
                                        "Device status changed from {} to {}",
                                        write.expected_status, write.status
                                    ),
                                })
                                .execute(connection)?;
                        }
                        let actions_enqueued =
                            enqueue_pending_actions(connection, &write.pending_actions)?;
                        Ok(DeviceWriteOutcome {
                            applied: true,
                            actions_enqueued,
                        })
                    })
                    .map_err(map_app_error)
            })
            .await
    }

    async fn offline_candidates(
        &self,
        cutoff: chrono::NaiveDateTime,
    ) -> Result<Vec<DeviceIngressContext>, PersistenceError> {
        self.executor
            .run(move |connection| {
                devices::table
                    .filter(devices::status.ne("offline"))
                    .filter(devices::last_seen.lt(cutoff))
                    .select(Device::as_select())
                    .load::<Device>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(|device| {
                        let blueprint_id =
                            blueprint_id_for_device(connection, &device.tenant_id, &device.id)
                                .map_err(map_diesel_error)?;
                        to_ingress_context(device, blueprint_id)
                    })
                    .collect()
            })
            .await
    }

    async fn apply_offline_transitions(
        &self,
        cutoff: chrono::NaiveDateTime,
        transitions: Vec<OfflineTransition>,
    ) -> Result<OfflineWriteOutcome, PersistenceError> {
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, AppError, _>(|connection| {
                        let mut devices_updated = 0;
                        let mut actions_enqueued = 0;
                        for transition in transitions {
                            let tenant_id = transition.context.identity.tenant_id_str();
                            let device_id = transition.context.identity.device_id();
                            let current = devices::table
                                .filter(devices::tenant_id.eq(tenant_id))
                                .filter(devices::id.eq(device_id))
                                .select((devices::status, devices::last_seen))
                                .for_update()
                                .first::<(String, Option<chrono::NaiveDateTime>)>(connection)
                                .optional()?;
                            let eligible = current.is_some_and(|(status, last_seen)| {
                                status == transition.context.status
                                    && status != "offline"
                                    && last_seen.is_some_and(|seen| seen < cutoff)
                            });
                            if !eligible {
                                continue;
                            }
                            diesel::update(
                                devices::table
                                    .filter(devices::tenant_id.eq(tenant_id))
                                    .filter(devices::id.eq(device_id)),
                            )
                            .set(devices::status.eq("offline"))
                            .execute(connection)?;
                            diesel::insert_into(device_logs::table)
                                .values(NewDeviceLog {
                                    tenant_id: tenant_id.to_string(),
                                    device_id: device_id.to_string(),
                                    level: "WARN".to_string(),
                                    message: "Device went offline (no heartbeat)".to_string(),
                                })
                                .execute(connection)?;
                            devices_updated += 1;
                            actions_enqueued +=
                                enqueue_pending_actions(connection, &transition.pending_actions)?;
                        }
                        Ok(OfflineWriteOutcome {
                            devices_updated,
                            actions_enqueued,
                        })
                    })
                    .map_err(map_app_error)
            })
            .await
    }

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
                        if let Some(contract) = contract {
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
