use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::Connection;
use diesel::PgTextExpressionMethods;
use diesel::prelude::*;
use diesel::sql_types::{Array, Jsonb, Text};
use serde_json::Value;

use crate::db::models::{
    Device, DeviceCertificate, DeviceType, Fleet, NetworkObservedHost, NewDevice,
    NewDeviceCertificate, NewDeviceShadow, UpdateDevice,
};
use crate::db::schema::{
    device_certificates, device_shadows, device_types, devices, fleets, network_observed_hosts,
};
use crate::domains::device_types::types::DeviceTypeRecord;
use crate::domains::devices::repository::DeviceRepository;
use crate::domains::devices::types::{
    CreateDeviceRecord, DeviceDetails, DeviceFilter, DeviceList, DeviceListQuery, DeviceRecord,
    UpdateDeviceRecord,
};
use crate::domains::fleets::types::FleetRecord;
use crate::domains::identity::certificate_types::NewDeviceCertificateRecord;
use crate::persistence::PersistenceError;
use crate::services::device_connections;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::PostgresAdapter;
use super::executor::map_diesel_error;

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
struct LatestTelemetryCustomJson {
    #[diesel(sql_type = Text)]
    device_id: String,
    #[diesel(sql_type = Jsonb)]
    custom_json: Value,
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

fn is_network_analyzer(device: &Device, device_type: &DeviceType) -> bool {
    device_type.name == "network-analyzer"
        || device.id.contains("network-analyzer")
        || device.name.contains("network-analyzer")
        || device.firmware.contains("network-analyzer")
        || device.firmware.contains("network_analyzer")
}

fn hydrate_connections(
    connection: &mut PgConnection,
    tenant_id: &str,
    rows: &mut [JoinedDevice],
) -> QueryResult<()> {
    let candidate_ids: Vec<String> = rows
        .iter()
        .filter(|(device, _, _)| {
            device
                .declared_connections
                .as_array()
                .is_none_or(Vec::is_empty)
        })
        .map(|(device, _, _)| device.id.clone())
        .collect();
    if !candidate_ids.is_empty() {
        let sources = diesel::sql_query(
            r#"
            SELECT DISTINCT ON (device_id) device_id, custom_json
            FROM telemetry
            WHERE tenant_id = $1
              AND device_id = ANY($2)
              AND custom_json->>'kind' = 'network_analyzer_scan'
              AND custom_json ? 'snapshot_json'
            ORDER BY device_id, received_at DESC, id DESC
            "#,
        )
        .bind::<Text, _>(tenant_id)
        .bind::<Array<Text>, _>(&candidate_ids)
        .load::<LatestTelemetryCustomJson>(connection)?;
        let connections: HashMap<String, Value> = sources
            .into_iter()
            .filter_map(|source| {
                device_connections::declared_connections_from_custom_json(&source.custom_json)
                    .map(|value| (source.device_id, value))
            })
            .collect();
        for (device, _, _) in &mut *rows {
            if device
                .declared_connections
                .as_array()
                .is_none_or(Vec::is_empty)
                && let Some(value) = connections.get(&device.id)
            {
                device.declared_connections = value.clone();
            }
        }
    }

    let analyzer_ids: Vec<String> = rows
        .iter()
        .filter(|(device, device_type, _)| is_network_analyzer(device, device_type))
        .map(|(device, _, _)| device.id.clone())
        .collect();
    if analyzer_ids.is_empty() {
        return Ok(());
    }
    let cutoff = Utc::now().naive_utc() - chrono::Duration::days(30);
    let hosts = network_observed_hosts::table
        .filter(network_observed_hosts::tenant_id.eq(tenant_id))
        .filter(network_observed_hosts::analyzer_device_id.eq_any(&analyzer_ids))
        .filter(network_observed_hosts::last_seen_at.ge(cutoff))
        .order((
            network_observed_hosts::analyzer_device_id.asc(),
            network_observed_hosts::status.asc(),
            network_observed_hosts::last_seen_at.desc(),
        ))
        .select(NetworkObservedHost::as_select())
        .load::<NetworkObservedHost>(connection)?;
    let mut by_analyzer: HashMap<String, Vec<Value>> = HashMap::new();
    for host in hosts {
        let observed = device_connections::ObservedNetworkHost {
            host_key: host.host_key,
            label: host.label,
            address: host.address,
            device_type: host.device_type,
            source: host.source,
        };
        by_analyzer
            .entry(host.analyzer_device_id)
            .or_default()
            .push(device_connections::network_host_connection(
                &observed,
                &host.status,
                Some(host.first_seen_at),
                Some(host.last_seen_at),
            ));
    }
    for (device, device_type, _) in rows {
        if is_network_analyzer(device, device_type)
            && let Some(connections) = by_analyzer.remove(&device.id)
        {
            device.declared_connections = Value::Array(connections);
        }
    }
    Ok(())
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
                let mut rows = filtered_query(&tenant_id, &filter)
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
                hydrate_connections(connection, &tenant_id, &mut rows).map_err(map_diesel_error)?;
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
                let mut rows = vec![row];
                hydrate_connections(connection, &tenant_id, &mut rows).map_err(map_diesel_error)?;
                Ok(rows.pop().map(to_details))
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
