// Repository functions for firmware

use diesel::prelude::*;
use diesel::SqliteConnection;

use crate::db::models::{
    DeviceType, FirmwareBlob, FirmwareUpdate, NewFirmwareBlob, NewFirmwareUpdate,
    NewOtaDeployment, OtaDeployment,
};
use crate::db::schema::{device_types, firmware_blobs, firmware_updates, ota_deployments};

pub fn list_firmware_updates(
    conn: &mut SqliteConnection,
    device_type_id: Option<i32>,
) -> Result<Vec<(FirmwareUpdate, DeviceType, Option<i32>, Option<String>)>, diesel::result::Error> {
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

    if let Some(dt_id) = device_type_id {
        query = query.filter(firmware_updates::device_type_id.eq(dt_id));
    }

    query
        .order(firmware_updates::created_at.desc())
        .load(conn)
}

pub fn find_firmware_update(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<FirmwareUpdate, diesel::result::Error> {
    firmware_updates::table
        .find(id)
        .select(FirmwareUpdate::as_select())
        .first(conn)
}

pub fn insert_firmware_update(
    conn: &mut SqliteConnection,
    record: &NewFirmwareUpdate,
) -> Result<FirmwareUpdate, diesel::result::Error> {
    diesel::insert_into(firmware_updates::table)
        .values(record)
        .execute(conn)?;

    firmware_updates::table
        .filter(
            firmware_updates::device_type_id
                .eq(record.device_type_id)
                .and(firmware_updates::version.eq(&record.version)),
        )
        .select(FirmwareUpdate::as_select())
        .first(conn)
}

pub fn delete_firmware_update(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(firmware_updates::table.find(id)).execute(conn)?;
    Ok(rows > 0)
}

pub fn insert_firmware_blob(
    conn: &mut SqliteConnection,
    blob: &NewFirmwareBlob,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(firmware_blobs::table)
        .values(blob)
        .execute(conn)?;
    Ok(())
}

pub fn find_firmware_blob(
    conn: &mut SqliteConnection,
    firmware_update_id: i32,
) -> Result<FirmwareBlob, diesel::result::Error> {
    firmware_blobs::table
        .find(firmware_update_id)
        .select(FirmwareBlob::as_select())
        .first(conn)
}

pub fn find_next_version(
    conn: &mut SqliteConnection,
    device_type_id: i32,
) -> Result<Option<String>, diesel::result::Error> {
    firmware_updates::table
        .filter(firmware_updates::device_type_id.eq(device_type_id))
        .select(firmware_updates::version)
        .order(firmware_updates::created_at.desc())
        .first(conn)
        .optional()
}

pub fn update_firmware_url(
    conn: &mut SqliteConnection,
    id: i32,
    url: &str,
) -> Result<(), diesel::result::Error> {
    diesel::update(firmware_updates::table.find(id))
        .set(firmware_updates::url.eq(url))
        .execute(conn)?;
    Ok(())
}

pub fn list_ota_deployments(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<Vec<(OtaDeployment, FirmwareUpdate)>, diesel::result::Error> {
    ota_deployments::table
        .inner_join(firmware_updates::table)
        .filter(ota_deployments::device_id.eq(device_id))
        .select((OtaDeployment::as_select(), FirmwareUpdate::as_select()))
        .order(ota_deployments::initiated_at.desc())
        .load(conn)
}

pub fn insert_ota_deployment(
    conn: &mut SqliteConnection,
    deployment: &NewOtaDeployment,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(ota_deployments::table)
        .values(deployment)
        .execute(conn)?;
    Ok(())
}
