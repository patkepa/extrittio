use async_trait::async_trait;
use chrono::{DateTime, Utc};
use turso::{Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::certificates::CertificateRepository;
use extrittio_backend_core::certificates::{
    CaCertificateRecord, CertificateMaterialOutcome, CertificateStatusOutcome,
    DeviceCertificateRecord, NewCaCertificateRecord, NewDeviceCertificateRecord,
    ReplaceCertificateOutcome, StoredPrivateKeyRecord,
};

use crate::row::legacy_error as map_error;
use crate::{TursoConnectionHandles, row};

#[derive(Clone)]
pub struct TursoCertificateRepository {
    handles: TursoConnectionHandles,
}
impl TursoCertificateRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}

fn decode_ca(record: &Row) -> Result<CaCertificateRecord, PersistenceError> {
    Ok(CaCertificateRecord {
        id: row::i32(
            record.get::<i64>(0).map_err(map_error)?,
            "ca_certificates.id",
        )?,
        private_key_pem: record.get(1).map_err(map_error)?,
        certificate_pem: record.get(2).map_err(map_error)?,
        created_at: row::datetime(record.get(3).map_err(map_error)?)?,
    })
}

fn decode_device(record: &Row) -> Result<DeviceCertificateRecord, PersistenceError> {
    Ok(DeviceCertificateRecord {
        id: row::i32(
            record.get::<i64>(0).map_err(map_error)?,
            "device_certificates.id",
        )?,
        device_id: record.get(1).map_err(map_error)?,
        private_key_pem: record.get(2).map_err(map_error)?,
        certificate_pem: record.get(3).map_err(map_error)?,
        fingerprint: record.get(4).map_err(map_error)?,
        expires_at: row::datetime(record.get(5).map_err(map_error)?)?,
        created_at: row::datetime(record.get(6).map_err(map_error)?)?,
    })
}

async fn ca_from(
    connection: &turso::Connection,
) -> Result<Option<CaCertificateRecord>, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT id, private_key_pem, certificate_pem, created_at
             FROM ca_certificates ORDER BY id DESC LIMIT 1",
            (),
        )
        .await
        .map_err(map_error)?;
    rows.next()
        .await
        .map_err(map_error)?
        .map(|record| decode_ca(&record))
        .transpose()
}

async fn device_from(
    connection: &turso::Connection,
    tenant: &TenantId,
    device_id: &str,
) -> Result<Option<DeviceCertificateRecord>, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT id, device_id, private_key_pem, certificate_pem, fingerprint,
                    expires_at, created_at FROM device_certificates
             WHERE tenant_id = ?1 AND device_id = ?2 ORDER BY id DESC LIMIT 1",
            params![tenant.as_str(), device_id],
        )
        .await
        .map_err(map_error)?;
    rows.next()
        .await
        .map_err(map_error)?
        .map(|record| decode_device(&record))
        .transpose()
}

async fn device_exists(
    connection: &turso::Connection,
    tenant: &TenantId,
    device_id: &str,
) -> Result<bool, PersistenceError> {
    let mut rows = connection
        .query(
            "SELECT EXISTS(SELECT 1 FROM devices WHERE tenant_id = ?1 AND id = ?2)",
            params![tenant.as_str(), device_id],
        )
        .await
        .map_err(map_error)?;
    Ok(rows
        .next()
        .await
        .map_err(map_error)?
        .ok_or(PersistenceError::NotFound)?
        .get::<i64>(0)
        .map_err(map_error)?
        != 0)
}

#[async_trait]
impl CertificateRepository for TursoCertificateRepository {
    async fn get_ca(&self) -> Result<Option<CaCertificateRecord>, PersistenceError> {
        ca_from(&self.connect()?).await
    }

    async fn insert_ca_if_absent(
        &self,
        record: NewCaCertificateRecord,
    ) -> Result<CaCertificateRecord, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        if let Some(existing) = ca_from(&transaction).await? {
            transaction.commit().await.map_err(map_error)?;
            return Ok(existing);
        }
        let mut rows = transaction
            .query(
                "INSERT INTO ca_certificates (private_key_pem, certificate_pem, created_at)
                 VALUES (?1, ?2, ?3)
                 RETURNING id, private_key_pem, certificate_pem, created_at",
                params![
                    record.private_key_pem,
                    record.certificate_pem,
                    Utc::now().timestamp_micros()
                ],
            )
            .await
            .map_err(map_error)?;
        let result = decode_ca(
            &rows
                .next()
                .await
                .map_err(map_error)?
                .ok_or(PersistenceError::NotFound)?,
        )?;
        drop(rows);
        transaction.commit().await.map_err(map_error)?;
        Ok(result)
    }

    async fn list_stored_private_keys(
        &self,
    ) -> Result<Vec<StoredPrivateKeyRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut result = Vec::new();
        let mut ca_rows = connection
            .query("SELECT id, private_key_pem FROM ca_certificates", ())
            .await
            .map_err(map_error)?;
        while let Some(record) = ca_rows.next().await.map_err(map_error)? {
            result.push(StoredPrivateKeyRecord::Ca {
                id: row::i32(record.get(0).map_err(map_error)?, "ca_certificates.id")?,
                value: record.get(1).map_err(map_error)?,
            });
        }
        let mut device_rows = connection
            .query(
                "SELECT tenant_id, id, private_key_pem FROM device_certificates
                 WHERE private_key_pem <> ''",
                (),
            )
            .await
            .map_err(map_error)?;
        while let Some(record) = device_rows.next().await.map_err(map_error)? {
            result.push(StoredPrivateKeyRecord::Device {
                tenant_id: record.get(0).map_err(map_error)?,
                id: row::i32(record.get(1).map_err(map_error)?, "device_certificates.id")?,
                value: record.get(2).map_err(map_error)?,
            });
        }
        Ok(result)
    }

    async fn replace_ca_private_key_if_matches(
        &self,
        certificate_id: i32,
        expected_value: String,
        replacement: String,
    ) -> Result<bool, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer
            .execute(
                "UPDATE ca_certificates SET private_key_pem = ?3 WHERE id = ?1 AND private_key_pem = ?2",
                params![i64::from(certificate_id), expected_value, replacement],
            )
            .await
            .map(|count| count > 0)
            .map_err(map_error)
    }

    async fn replace_device_private_key_if_matches(
        &self,
        tenant_id: String,
        certificate_id: i32,
        expected_value: String,
        replacement: String,
    ) -> Result<bool, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer
            .execute(
                "UPDATE device_certificates SET private_key_pem = ?4
                 WHERE tenant_id = ?1 AND id = ?2 AND private_key_pem = ?3",
                params![
                    tenant_id,
                    i64::from(certificate_id),
                    expected_value,
                    replacement
                ],
            )
            .await
            .map(|count| count > 0)
            .map_err(map_error)
    }

    async fn get_download_material(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<CertificateMaterialOutcome, PersistenceError> {
        let connection = self.connect()?;
        if !device_exists(&connection, tenant, device_id).await? {
            return Ok(CertificateMaterialOutcome::DeviceNotFound);
        }
        let Some(device) = device_from(&connection, tenant, device_id).await? else {
            return Ok(CertificateMaterialOutcome::CertificateNotFound);
        };
        let Some(ca) = ca_from(&connection).await? else {
            return Ok(CertificateMaterialOutcome::CaNotFound);
        };
        Ok(CertificateMaterialOutcome::Found { device, ca })
    }

    async fn consume_private_key(
        &self,
        tenant: &TenantId,
        certificate_id: i32,
        expected_value: String,
    ) -> Result<bool, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer
            .execute(
                "UPDATE device_certificates SET private_key_pem = ''
                 WHERE tenant_id = ?1 AND id = ?2 AND private_key_pem = ?3",
                params![tenant.as_str(), i64::from(certificate_id), expected_value],
            )
            .await
            .map(|count| count > 0)
            .map_err(map_error)
    }

    async fn get_status(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<CertificateStatusOutcome, PersistenceError> {
        let connection = self.connect()?;
        if !device_exists(&connection, tenant, device_id).await? {
            return Ok(CertificateStatusOutcome::DeviceNotFound);
        }
        Ok(CertificateStatusOutcome::Found(
            device_from(&connection, tenant, device_id).await?,
        ))
    }

    async fn replace_device_certificate(
        &self,
        tenant: &TenantId,
        record: NewDeviceCertificateRecord,
    ) -> Result<ReplaceCertificateOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        if !device_exists(&transaction, tenant, &record.device_id).await? {
            transaction.rollback().await.map_err(map_error)?;
            return Ok(ReplaceCertificateOutcome::DeviceNotFound);
        }
        transaction
            .execute(
                "DELETE FROM device_certificates WHERE tenant_id = ?1 AND device_id = ?2",
                params![tenant.as_str(), record.device_id.clone()],
            )
            .await
            .map_err(map_error)?;
        let mut rows = transaction
            .query(
                "INSERT INTO device_certificates
                   (tenant_id, device_id, private_key_pem, certificate_pem, fingerprint,
                    expires_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 RETURNING id, device_id, private_key_pem, certificate_pem, fingerprint,
                           expires_at, created_at",
                params![
                    tenant.as_str(),
                    record.device_id,
                    record.private_key_pem,
                    record.certificate_pem,
                    record.fingerprint,
                    record.expires_at.timestamp_micros(),
                    Utc::now().timestamp_micros()
                ],
            )
            .await
            .map_err(map_error)?;
        let result = decode_device(
            &rows
                .next()
                .await
                .map_err(map_error)?
                .ok_or(PersistenceError::NotFound)?,
        )?;
        drop(rows);
        // The regenerate response consumes the new key in this same transaction.
        transaction.execute(
            "UPDATE device_certificates SET private_key_pem = '' WHERE tenant_id = ?1 AND id = ?2",
            params![tenant.as_str(), result.id],
        ).await.map_err(map_error)?;
        transaction.commit().await.map_err(map_error)?;
        Ok(ReplaceCertificateOutcome::Replaced(result))
    }

    async fn list_active_device_ids(
        &self,
        active_at: DateTime<Utc>,
    ) -> Result<Vec<String>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT DISTINCT device_id FROM device_certificates
                 WHERE expires_at > ?1 ORDER BY device_id",
                params![active_at.timestamp_micros()],
            )
            .await
            .map_err(map_error)?;
        let mut ids = Vec::new();
        while let Some(record) = rows.next().await.map_err(map_error)? {
            ids.push(record.get(0).map_err(map_error)?);
        }
        Ok(ids)
    }
}
