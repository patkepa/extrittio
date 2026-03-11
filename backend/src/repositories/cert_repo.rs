use diesel::SqliteConnection;
use diesel::prelude::*;

use crate::db::models::{CaCertificate, DeviceCertificate, NewCaCertificate, NewDeviceCertificate};
use crate::db::schema::{ca_certificates, device_certificates};

pub fn get_ca_certificate(
    conn: &mut SqliteConnection,
) -> Result<Option<CaCertificate>, diesel::result::Error> {
    ca_certificates::table
        .select(CaCertificate::as_select())
        .order(ca_certificates::id.desc())
        .first(conn)
        .optional()
}

pub fn insert_ca_certificate(
    conn: &mut SqliteConnection,
    ca: &NewCaCertificate,
) -> Result<CaCertificate, diesel::result::Error> {
    diesel::insert_into(ca_certificates::table)
        .values(ca)
        .execute(conn)?;

    ca_certificates::table
        .select(CaCertificate::as_select())
        .order(ca_certificates::id.desc())
        .first(conn)
}

pub fn get_device_certificate(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<Option<DeviceCertificate>, diesel::result::Error> {
    device_certificates::table
        .filter(device_certificates::device_id.eq(device_id))
        .select(DeviceCertificate::as_select())
        .order(device_certificates::id.desc())
        .first(conn)
        .optional()
}

pub fn insert_device_certificate(
    conn: &mut SqliteConnection,
    cert: &NewDeviceCertificate,
) -> Result<DeviceCertificate, diesel::result::Error> {
    diesel::insert_into(device_certificates::table)
        .values(cert)
        .execute(conn)?;

    device_certificates::table
        .filter(device_certificates::device_id.eq(&cert.device_id))
        .select(DeviceCertificate::as_select())
        .order(device_certificates::id.desc())
        .first(conn)
}

pub fn clear_device_private_key(
    conn: &mut SqliteConnection,
    cert_id: i32,
) -> Result<(), diesel::result::Error> {
    diesel::update(device_certificates::table.find(cert_id))
        .set(device_certificates::private_key_pem.eq(""))
        .execute(conn)?;
    Ok(())
}

pub fn delete_device_certificates(
    conn: &mut SqliteConnection,
    device_id: &str,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(device_certificates::table.filter(device_certificates::device_id.eq(device_id)))
        .execute(conn)
}
