use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::Connection;
use diesel::prelude::*;

use crate::db::models::{CaCertificate, DeviceCertificate, NewCaCertificate, NewDeviceCertificate};
use crate::db::schema::{ca_certificates, device_certificates, devices};
use crate::domains::identity::certificate_repository::CertificateRepository;
use crate::domains::identity::certificate_types::{
    CaCertificateRecord, CertificateMaterialOutcome, CertificateStatusOutcome,
    DeviceCertificateRecord, NewCaCertificateRecord, NewDeviceCertificateRecord,
    ReplaceCertificateOutcome, StoredPrivateKeyRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

fn to_ca_record(row: CaCertificate) -> CaCertificateRecord {
    CaCertificateRecord {
        id: row.id,
        private_key_pem: row.private_key_pem,
        certificate_pem: row.certificate_pem,
        created_at: row.created_at.and_utc(),
    }
}

fn to_device_record(row: DeviceCertificate) -> DeviceCertificateRecord {
    DeviceCertificateRecord {
        id: row.id,
        device_id: row.device_id,
        private_key_pem: row.private_key_pem,
        certificate_pem: row.certificate_pem,
        fingerprint: row.fingerprint,
        expires_at: row.expires_at.and_utc(),
        created_at: row.created_at.and_utc(),
    }
}

#[async_trait]
impl CertificateRepository for PostgresAdapter {
    async fn get_ca(&self) -> Result<Option<CaCertificateRecord>, PersistenceError> {
        self.executor
            .run(move |connection| {
                ca_certificates::table
                    .select(CaCertificate::as_select())
                    .order(ca_certificates::id.desc())
                    .first::<CaCertificate>(connection)
                    .optional()
                    .map(|row| row.map(to_ca_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn insert_ca_if_absent(
        &self,
        record: NewCaCertificateRecord,
    ) -> Result<CaCertificateRecord, PersistenceError> {
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        diesel::sql_query("LOCK TABLE ca_certificates IN EXCLUSIVE MODE")
                            .execute(connection)?;
                        let existing = ca_certificates::table
                            .select(CaCertificate::as_select())
                            .order(ca_certificates::id.desc())
                            .for_update()
                            .first::<CaCertificate>(connection)
                            .optional()?;
                        let row = match existing {
                            Some(row) => row,
                            None => diesel::insert_into(ca_certificates::table)
                                .values(NewCaCertificate {
                                    private_key_pem: record.private_key_pem,
                                    certificate_pem: record.certificate_pem,
                                })
                                .returning(CaCertificate::as_returning())
                                .get_result::<CaCertificate>(connection)?,
                        };
                        Ok(to_ca_record(row))
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn list_stored_private_keys(
        &self,
    ) -> Result<Vec<StoredPrivateKeyRecord>, PersistenceError> {
        self.executor
            .run(move |connection| {
                let ca = ca_certificates::table
                    .filter(ca_certificates::private_key_pem.ne(""))
                    .select((ca_certificates::id, ca_certificates::private_key_pem))
                    .load::<(i32, String)>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(|(id, value)| StoredPrivateKeyRecord::Ca { id, value });
                let devices = device_certificates::table
                    .filter(device_certificates::private_key_pem.ne(""))
                    .select((
                        device_certificates::tenant_id,
                        device_certificates::id,
                        device_certificates::private_key_pem,
                    ))
                    .load::<(String, i32, String)>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(|(tenant_id, id, value)| StoredPrivateKeyRecord::Device {
                        tenant_id,
                        id,
                        value,
                    });
                Ok(ca.chain(devices).collect())
            })
            .await
    }

    async fn replace_ca_private_key_if_matches(
        &self,
        certificate_id: i32,
        expected_value: String,
        replacement: String,
    ) -> Result<bool, PersistenceError> {
        self.executor
            .run(move |connection| {
                diesel::update(
                    ca_certificates::table
                        .filter(ca_certificates::id.eq(certificate_id))
                        .filter(ca_certificates::private_key_pem.eq(expected_value)),
                )
                .set(ca_certificates::private_key_pem.eq(replacement))
                .execute(connection)
                .map(|rows| rows == 1)
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn replace_device_private_key_if_matches(
        &self,
        tenant_id: String,
        certificate_id: i32,
        expected_value: String,
        replacement: String,
    ) -> Result<bool, PersistenceError> {
        self.executor
            .run(move |connection| {
                diesel::update(
                    device_certificates::table
                        .filter(device_certificates::tenant_id.eq(tenant_id))
                        .filter(device_certificates::id.eq(certificate_id))
                        .filter(device_certificates::private_key_pem.eq(expected_value)),
                )
                .set(device_certificates::private_key_pem.eq(replacement))
                .execute(connection)
                .map(|rows| rows == 1)
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_download_material(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<CertificateMaterialOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                let exists = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .filter(devices::id.eq(&device_id))
                    .select(devices::id)
                    .first::<String>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                if exists.is_none() {
                    return Ok(CertificateMaterialOutcome::DeviceNotFound);
                }
                let certificate = device_certificates::table
                    .filter(device_certificates::tenant_id.eq(&tenant_id))
                    .filter(device_certificates::device_id.eq(&device_id))
                    .select(DeviceCertificate::as_select())
                    .order(device_certificates::id.desc())
                    .first::<DeviceCertificate>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                let Some(certificate) = certificate else {
                    return Ok(CertificateMaterialOutcome::CertificateNotFound);
                };
                let ca = ca_certificates::table
                    .select(CaCertificate::as_select())
                    .order(ca_certificates::id.desc())
                    .first::<CaCertificate>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                let Some(ca) = ca else {
                    return Ok(CertificateMaterialOutcome::CaNotFound);
                };
                Ok(CertificateMaterialOutcome::Found {
                    device: to_device_record(certificate),
                    ca: to_ca_record(ca),
                })
            })
            .await
    }

    async fn consume_private_key(
        &self,
        tenant: &TenantId,
        certificate_id: i32,
        expected_value: String,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::update(
                    device_certificates::table
                        .filter(device_certificates::tenant_id.eq(tenant_id))
                        .filter(device_certificates::id.eq(certificate_id))
                        .filter(device_certificates::private_key_pem.eq(expected_value)),
                )
                .set(device_certificates::private_key_pem.eq(""))
                .execute(connection)
                .map(|rows| rows == 1)
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_status(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<CertificateStatusOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                let exists = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .filter(devices::id.eq(&device_id))
                    .select(devices::id)
                    .first::<String>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                if exists.is_none() {
                    return Ok(CertificateStatusOutcome::DeviceNotFound);
                }
                let certificate = device_certificates::table
                    .filter(device_certificates::tenant_id.eq(tenant_id))
                    .filter(device_certificates::device_id.eq(device_id))
                    .select(DeviceCertificate::as_select())
                    .order(device_certificates::id.desc())
                    .first::<DeviceCertificate>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;
                Ok(CertificateStatusOutcome::Found(
                    certificate.map(to_device_record),
                ))
            })
            .await
    }

    async fn replace_device_certificate(
        &self,
        tenant: &TenantId,
        record: NewDeviceCertificateRecord,
    ) -> Result<ReplaceCertificateOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let exists = devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::id.eq(&record.device_id))
                            .select(devices::id)
                            .for_update()
                            .first::<String>(connection)
                            .optional()?;
                        if exists.is_none() {
                            return Ok(ReplaceCertificateOutcome::DeviceNotFound);
                        }
                        diesel::delete(
                            device_certificates::table
                                .filter(device_certificates::tenant_id.eq(&tenant_id))
                                .filter(device_certificates::device_id.eq(&record.device_id)),
                        )
                        .execute(connection)?;
                        let certificate = diesel::insert_into(device_certificates::table)
                            .values(NewDeviceCertificate {
                                tenant_id: tenant_id.clone(),
                                device_id: record.device_id,
                                private_key_pem: record.private_key_pem,
                                certificate_pem: record.certificate_pem,
                                fingerprint: record.fingerprint,
                                expires_at: record.expires_at.naive_utc(),
                            })
                            .returning(DeviceCertificate::as_returning())
                            .get_result::<DeviceCertificate>(connection)?;
                        diesel::update(
                            device_certificates::table
                                .filter(device_certificates::tenant_id.eq(&tenant_id))
                                .filter(device_certificates::id.eq(certificate.id)),
                        )
                        .set(device_certificates::private_key_pem.eq(""))
                        .execute(connection)?;
                        Ok(ReplaceCertificateOutcome::Replaced(to_device_record(
                            certificate,
                        )))
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn list_active_device_ids(
        &self,
        active_at: DateTime<Utc>,
    ) -> Result<Vec<String>, PersistenceError> {
        self.executor
            .run(move |connection| {
                device_certificates::table
                    .filter(device_certificates::expires_at.gt(active_at.naive_utc()))
                    .select(device_certificates::device_id)
                    .distinct()
                    .order(device_certificates::device_id.asc())
                    .load(connection)
                    .map_err(map_diesel_error)
            })
            .await
    }
}
