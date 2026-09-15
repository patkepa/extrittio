// Repository functions for firmware

use diesel::PgConnection;
use diesel::prelude::*;

use crate::models::{
    Device, FirmwareBlob, FirmwareUpdate, Fleet, NewFirmwareBlob, NewFirmwareUpdate,
    NewOtaDeployment, OtaDeployment,
};
use crate::schema::{devices, firmware_blobs, firmware_updates, fleets, ota_deployments};

pub type FirmwareUpdateRow = (FirmwareUpdate, Option<i32>, Option<String>);
pub type OtaDeploymentGlobalRow = (OtaDeployment, FirmwareUpdate, Device, Option<Fleet>);

pub fn list_firmware_updates(
    conn: &mut PgConnection,
    tenant_id: &str,
    blueprint_revision_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FirmwareUpdateRow>, i64), diesel::result::Error> {
    // Count query
    let mut count_query = firmware_updates::table.into_boxed();
    count_query = count_query.filter(firmware_updates::tenant_id.eq(tenant_id));
    if let Some(revision_id) = blueprint_revision_id {
        count_query = count_query.filter(firmware_updates::blueprint_revision_id.eq(revision_id));
    }
    let total: i64 = count_query.count().get_result(conn)?;

    // Data query
    let mut query = firmware_updates::table
        .left_join(firmware_blobs::table)
        .select((
            FirmwareUpdate::as_select(),
            firmware_blobs::size.nullable(),
            firmware_blobs::filename.nullable(),
        ))
        .into_boxed();
    query = query.filter(firmware_updates::tenant_id.eq(tenant_id));

    if let Some(revision_id) = blueprint_revision_id {
        query = query.filter(firmware_updates::blueprint_revision_id.eq(revision_id));
    }

    let results = query
        .order((
            firmware_updates::created_at.desc(),
            firmware_updates::id.desc(),
        ))
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
                    firmware_updates::blueprint_revision_id
                        .eq(&record.blueprint_revision_id)
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

pub fn find_optional_firmware_blob(
    conn: &mut PgConnection,
    tenant_id: &str,
    firmware_update_id: i32,
) -> Result<Option<FirmwareBlob>, diesel::result::Error> {
    firmware_blobs::table
        .filter(firmware_blobs::tenant_id.eq(tenant_id))
        .filter(firmware_blobs::firmware_update_id.eq(firmware_update_id))
        .select(FirmwareBlob::as_select())
        .first(conn)
        .optional()
}

pub fn find_next_blueprint_version(
    conn: &mut PgConnection,
    tenant_id: &str,
    blueprint_revision_id: &str,
) -> Result<Option<String>, diesel::result::Error> {
    firmware_updates::table
        .filter(firmware_updates::tenant_id.eq(tenant_id))
        .filter(firmware_updates::blueprint_revision_id.eq(blueprint_revision_id))
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
        .order((
            ota_deployments::initiated_at.desc(),
            ota_deployments::id.desc(),
        ))
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
        .inner_join(
            firmware_updates::table.on(firmware_updates::id
                .eq(ota_deployments::firmware_update_id)
                .and(firmware_updates::tenant_id.eq(ota_deployments::tenant_id))),
        )
        .inner_join(
            devices::table
                .left_join(
                    fleets::table.on(fleets::id
                        .nullable()
                        .eq(devices::fleet_id)
                        .and(fleets::tenant_id.eq(devices::tenant_id))),
                )
                .on(devices::id
                    .eq(ota_deployments::device_id)
                    .and(devices::tenant_id.eq(ota_deployments::tenant_id))),
        )
        .select((
            OtaDeployment::as_select(),
            FirmwareUpdate::as_select(),
            Device::as_select(),
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

/// Update OTA deployment status. If terminal (success/failed), also set
/// completed_at and optional error_message.
pub fn update_ota_deployment_status(
    conn: &mut PgConnection,
    tenant_id: &str,
    deployment_id: i32,
    status: &str,
    error_message: Option<&str>,
    completed_at: Option<chrono::NaiveDateTime>,
) -> Result<usize, diesel::result::Error> {
    diesel::update(
        ota_deployments::table
            .filter(ota_deployments::tenant_id.eq(tenant_id))
            .filter(ota_deployments::id.eq(deployment_id)),
    )
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
) -> Result<i32, diesel::result::Error> {
    diesel::insert_into(ota_deployments::table)
        .values(deployment)
        .returning(ota_deployments::id)
        .get_result(conn)
}

pub fn find_device_for_tenant(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Device, diesel::result::Error> {
    devices::table
        .filter(devices::tenant_id.eq(tenant))
        .filter(devices::id.eq(id))
        .select(Device::as_select())
        .first(conn)
}
pub fn blueprint_revision_exists(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::device_blueprint_revisions as revisions;
    diesel::select(diesel::dsl::exists(
        revisions::table
            .filter(revisions::tenant_id.eq(tenant))
            .filter(revisions::id.eq(id)),
    ))
    .get_result(conn)
}
