use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{CaCertificate, DeviceCertificate, NewCaCertificate, NewDeviceCertificate};
use crate::db::schema::{ca_certificates, device_certificates};
use crate::tenancy::DEFAULT_TENANT_ID;

pub fn get_ca_certificate(
    conn: &mut PgConnection,
) -> Result<Option<CaCertificate>, diesel::result::Error> {
    ca_certificates::table
        .select(CaCertificate::as_select())
        .order(ca_certificates::id.desc())
        .first(conn)
        .optional()
}

pub fn insert_ca_certificate(
    conn: &mut PgConnection,
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

pub fn update_ca_private_key(
    conn: &mut PgConnection,
    ca_id: i32,
    private_key_pem: &str,
) -> Result<(), diesel::result::Error> {
    diesel::update(ca_certificates::table.filter(ca_certificates::id.eq(ca_id)))
        .set(ca_certificates::private_key_pem.eq(private_key_pem))
        .execute(conn)?;
    Ok(())
}

pub fn get_device_certificate(
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<Option<DeviceCertificate>, diesel::result::Error> {
    get_device_certificate_for_tenant(conn, DEFAULT_TENANT_ID, device_id)
}

pub fn get_device_certificate_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
) -> Result<Option<DeviceCertificate>, diesel::result::Error> {
    device_certificates::table
        .filter(device_certificates::tenant_id.eq(tenant_id))
        .filter(device_certificates::device_id.eq(device_id))
        .select(DeviceCertificate::as_select())
        .order(device_certificates::id.desc())
        .first(conn)
        .optional()
}

pub fn insert_device_certificate(
    conn: &mut PgConnection,
    cert: &NewDeviceCertificate,
) -> Result<DeviceCertificate, diesel::result::Error> {
    diesel::insert_into(device_certificates::table)
        .values(cert)
        .execute(conn)?;

    device_certificates::table
        .filter(device_certificates::tenant_id.eq(&cert.tenant_id))
        .filter(device_certificates::device_id.eq(&cert.device_id))
        .select(DeviceCertificate::as_select())
        .order(device_certificates::id.desc())
        .first(conn)
}

pub fn list_device_certificates_with_private_keys(
    conn: &mut PgConnection,
) -> Result<Vec<DeviceCertificate>, diesel::result::Error> {
    device_certificates::table
        .filter(device_certificates::private_key_pem.ne(""))
        .select(DeviceCertificate::as_select())
        .load(conn)
}

pub fn list_active_device_certificate_device_ids(
    conn: &mut PgConnection,
) -> Result<Vec<String>, diesel::result::Error> {
    device_certificates::table
        .filter(device_certificates::expires_at.gt(chrono::Utc::now().naive_utc()))
        .select(device_certificates::device_id)
        .distinct()
        .load(conn)
}

pub fn update_device_private_key_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    cert_id: i32,
    private_key_pem: &str,
) -> Result<(), diesel::result::Error> {
    diesel::update(
        device_certificates::table
            .filter(device_certificates::tenant_id.eq(tenant_id))
            .filter(device_certificates::id.eq(cert_id)),
    )
    .set(device_certificates::private_key_pem.eq(private_key_pem))
    .execute(conn)?;
    Ok(())
}

pub fn clear_device_private_key(
    conn: &mut PgConnection,
    cert_id: i32,
) -> Result<(), diesel::result::Error> {
    clear_device_private_key_for_tenant(conn, DEFAULT_TENANT_ID, cert_id)
}

pub fn clear_device_private_key_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    cert_id: i32,
) -> Result<(), diesel::result::Error> {
    diesel::update(
        device_certificates::table
            .filter(device_certificates::tenant_id.eq(tenant_id))
            .filter(device_certificates::id.eq(cert_id)),
    )
    .set(device_certificates::private_key_pem.eq(""))
    .execute(conn)?;
    Ok(())
}

pub fn delete_device_certificates(
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<usize, diesel::result::Error> {
    delete_device_certificates_for_tenant(conn, DEFAULT_TENANT_ID, device_id)
}

pub fn delete_device_certificates_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    device_id: &str,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(
        device_certificates::table
            .filter(device_certificates::tenant_id.eq(tenant_id))
            .filter(device_certificates::device_id.eq(device_id)),
    )
    .execute(conn)
}
