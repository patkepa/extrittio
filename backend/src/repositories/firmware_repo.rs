// Repository functions for firmware

use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{
    Device, DeviceType, FirmwareBlob, FirmwareUpdate, Fleet, NewFirmwareBlob, NewFirmwareUpdate,
    NewOtaDeployment, OtaDeployment,
};
use crate::db::schema::{
    device_types, devices, firmware_blobs, firmware_updates, fleets, ota_deployments,
};

pub type FirmwareUpdateRow = (FirmwareUpdate, DeviceType, Option<i32>, Option<String>);
pub type OtaDeploymentGlobalRow = (
    OtaDeployment,
    FirmwareUpdate,
    Device,
    DeviceType,
    Option<Fleet>,
);

pub fn list_firmware_updates(
    conn: &mut PgConnection,
    tenant_id: &str,
    device_type_id: Option<i32>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FirmwareUpdateRow>, i64), diesel::result::Error> {
    // Count query
    let mut count_query = firmware_updates::table.into_boxed();
    count_query = count_query.filter(firmware_updates::tenant_id.eq(tenant_id));
    if let Some(dt_id) = device_type_id {
        count_query = count_query.filter(firmware_updates::device_type_id.eq(dt_id));
    }
    let total: i64 = count_query.count().get_result(conn)?;

    // Data query
    let mut query = firmware_updates::table
        .inner_join(device_types::table)
        .left_join(firmware_blobs::table)
        .select((
            FirmwareUpdate::as_select(),
            DeviceType::as_select(),
            firmware_blobs::size.nullable(),
            firmware_blobs::filename.nullable(),
        ))
        .into_boxed();
    query = query.filter(firmware_updates::tenant_id.eq(tenant_id));

    if let Some(dt_id) = device_type_id {
        query = query.filter(firmware_updates::device_type_id.eq(dt_id));
    }

    let results = query
        .order(firmware_updates::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)?;

    Ok((results, total))
}

pub fn find_firmware_update(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: i32,
) -> Result<FirmwareUpdate, diesel::result::Error> {
    firmware_updates::table
        .filter(firmware_updates::tenant_id.eq(tenant_id))
        .filter(firmware_updates::id.eq(id))
        .select(FirmwareUpdate::as_select())
        .first(conn)
}

pub fn insert_firmware_update(
    conn: &mut PgConnection,
    tenant_id: &str,
    record: &NewFirmwareUpdate,
) -> Result<FirmwareUpdate, diesel::result::Error> {
    use diesel::Connection;

    conn.transaction(|conn| {
        diesel::insert_into(firmware_updates::table)
            .values(record)
            .execute(conn)?;

        firmware_updates::table
            .filter(
                firmware_updates::tenant_id.eq(tenant_id).and(
                    firmware_updates::device_type_id
                        .eq(record.device_type_id)
                        .and(firmware_updates::version.eq(&record.version)),
                ),
            )
            .select(FirmwareUpdate::as_select())
            .first(conn)
    })
}

pub fn delete_firmware_update(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: i32,
) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(
        firmware_updates::table
            .filter(firmware_updates::tenant_id.eq(tenant_id))
            .filter(firmware_updates::id.eq(id)),
    )
    .execute(conn)?;
    Ok(rows > 0)
}

pub fn insert_firmware_blob(
    conn: &mut PgConnection,
    blob: &NewFirmwareBlob,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(firmware_blobs::table)
        .values(blob)
        .execute(conn)?;
    Ok(())
}

pub fn find_firmware_blob(
    conn: &mut PgConnection,
    firmware_update_id: i32,
) -> Result<FirmwareBlob, diesel::result::Error> {
    firmware_blobs::table
        .find(firmware_update_id)
        .select(FirmwareBlob::as_select())
        .first(conn)
}

pub fn find_next_version(
    conn: &mut PgConnection,
    tenant_id: &str,
    device_type_id: i32,
) -> Result<Option<String>, diesel::result::Error> {
    firmware_updates::table
        .filter(firmware_updates::tenant_id.eq(tenant_id))
        .filter(firmware_updates::device_type_id.eq(device_type_id))
        .select(firmware_updates::version)
        .order(firmware_updates::created_at.desc())
        .first(conn)
        .optional()
}

pub fn update_firmware_url(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: i32,
    url: &str,
) -> Result<(), diesel::result::Error> {
    diesel::update(
        firmware_updates::table
            .filter(firmware_updates::tenant_id.eq(tenant_id))
            .filter(firmware_updates::id.eq(id)),
    )
    .set(firmware_updates::url.eq(url))
    .execute(conn)?;
    Ok(())
}

pub fn list_ota_deployments(
    conn: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<(OtaDeployment, FirmwareUpdate)>, i64), diesel::result::Error> {
    let total: i64 = ota_deployments::table
        .filter(ota_deployments::tenant_id.eq(tenant_id))
        .filter(ota_deployments::device_id.eq(device_id))
        .count()
        .get_result(conn)?;

    let results = ota_deployments::table
        .inner_join(firmware_updates::table)
        .filter(ota_deployments::tenant_id.eq(tenant_id))
        .filter(ota_deployments::device_id.eq(device_id))
        .select((OtaDeployment::as_select(), FirmwareUpdate::as_select()))
        .order(ota_deployments::initiated_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)?;

    Ok((results, total))
}

pub fn list_all_ota_deployments(
    conn: &mut PgConnection,
    tenant_id: &str,
    status_filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<OtaDeploymentGlobalRow>, i64), diesel::result::Error> {
    let mut count_query = ota_deployments::table.into_boxed();
    count_query = count_query.filter(ota_deployments::tenant_id.eq(tenant_id));
    let mut query = ota_deployments::table
        .inner_join(firmware_updates::table)
        .inner_join(
            devices::table
                .inner_join(device_types::table)
                .left_join(fleets::table),
        )
        .select((
            OtaDeployment::as_select(),
            FirmwareUpdate::as_select(),
            Device::as_select(),
            DeviceType::as_select(),
            Option::<Fleet>::as_select(),
        ))
        .into_boxed();
    query = query.filter(ota_deployments::tenant_id.eq(tenant_id));

    match status_filter {
        Some("in_progress" | "active") => {
            count_query = count_query
                .filter(ota_deployments::status.ne("success"))
                .filter(ota_deployments::status.ne("failed"));
            query = query
                .filter(ota_deployments::status.ne("success"))
                .filter(ota_deployments::status.ne("failed"));
        }
        Some("completed" | "terminal") => {
            count_query = count_query.filter(
                ota_deployments::status
                    .eq("success")
                    .or(ota_deployments::status.eq("failed")),
            );
            query = query.filter(
                ota_deployments::status
                    .eq("success")
                    .or(ota_deployments::status.eq("failed")),
            );
        }
        Some(status) if !status.is_empty() && status != "all" => {
            count_query = count_query.filter(ota_deployments::status.eq(status));
            query = query.filter(ota_deployments::status.eq(status));
        }
        _ => {}
    }

    let total: i64 = count_query.count().get_result(conn)?;
    let results = query
        .order(ota_deployments::initiated_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)?;

    Ok((results, total))
}

/// Find the ID of the active (non-terminal) OTA deployment for the given device,
/// optionally filtered by firmware_update_id for precise targeting.
pub fn find_active_ota_deployment(
    conn: &mut PgConnection,
    device_id: &str,
    firmware_update_id: Option<i32>,
) -> Result<Option<i32>, diesel::result::Error> {
    let mut query = ota_deployments::table
        .filter(ota_deployments::device_id.eq(device_id))
        .filter(ota_deployments::status.ne("success"))
        .filter(ota_deployments::status.ne("failed"))
        .order(ota_deployments::initiated_at.desc())
        .select(ota_deployments::id)
        .into_boxed();

    if let Some(fwid) = firmware_update_id {
        query = query.filter(ota_deployments::firmware_update_id.eq(fwid));
    }

    query.first(conn).optional()
}

/// Update OTA deployment status. If terminal (success/failed), also set
/// completed_at and optional error_message.
pub fn update_ota_deployment_status(
    conn: &mut PgConnection,
    deployment_id: i32,
    status: &str,
    error_message: Option<&str>,
    completed_at: Option<chrono::NaiveDateTime>,
) -> Result<usize, diesel::result::Error> {
    diesel::update(ota_deployments::table.find(deployment_id))
        .set((
            ota_deployments::status.eq(status),
            ota_deployments::error_message.eq(error_message),
            ota_deployments::completed_at.eq(completed_at),
        ))
        .execute(conn)
}

pub fn insert_ota_deployment(
    conn: &mut PgConnection,
    deployment: &NewOtaDeployment,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(ota_deployments::table)
        .values(deployment)
        .execute(conn)?;
    Ok(())
}
