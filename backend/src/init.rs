use std::time::Duration;

use anyhow::Context;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::MigrationHarness;
use tracing::{debug, info};

use crate::db::models::{NewDeviceType, NewServerConfigEntry, NewUser, ServerConfigEntry};
use crate::db::schema::{ca_certificates, device_types, server_config, users};
use crate::repositories::cert_repo;
use crate::services::cert_service;
use crate::state::DbPool;
use crate::{MIGRATIONS, auth};

/// Create the PostgreSQL connection pool.
pub fn create_db_pool(database_url: &str, pool_size: u32) -> anyhow::Result<DbPool> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .max_size(pool_size)
        .connection_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(300)))
        .build(manager)
        .context("Failed to create database connection pool")
}

/// Run pending migrations.
pub fn run_migrations(conn: &mut PgConnection) -> anyhow::Result<()> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to run database migrations: {e}"))?;

    info!("Database migrations completed successfully");
    Ok(())
}

/// Ensure built-in device types exist even if a dev/test database was reseeded
/// after migrations had already run.
pub fn seed_default_device_types(conn: &mut PgConnection) -> anyhow::Result<()> {
    const BUILT_IN_DEVICE_TYPES: &[(&str, &str, &str)] = &[
        ("default", "cube", "#8ABBFF"),
        ("mac-device", "desktop", "#F7C948"),
        ("network-analyzer", "antenna", "#36CFC9"),
    ];

    let rows: Vec<NewDeviceType> = BUILT_IN_DEVICE_TYPES
        .iter()
        .map(|(name, icon, color_hex)| NewDeviceType {
            tenant_id: crate::tenancy::DEFAULT_TENANT_ID.to_string(),
            name: (*name).to_string(),
            icon: (*icon).to_string(),
            color_hex: (*color_hex).to_string(),
        })
        .collect();

    diesel::insert_into(device_types::table)
        .values(&rows)
        .on_conflict((device_types::tenant_id, device_types::name))
        .do_nothing()
        .execute(conn)
        .context("Failed to seed built-in device types")?;

    Ok(())
}

/// Initialize JWT secret from the database, or generate one if not present.
/// Falls back to the `JWT_SECRET` environment variable if set.
pub fn init_jwt_secret(conn: &mut PgConnection) -> anyhow::Result<String> {
    let jwt_secret = {
        let existing: Option<ServerConfigEntry> = server_config::table
            .find("jwt_secret")
            .select(ServerConfigEntry::as_select())
            .first(conn)
            .optional()
            .context("Failed to query server_config")?;

        if let Some(entry) = existing {
            entry.value
        } else {
            use rand::Rng;
            let secret: String = rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(64)
                .map(char::from)
                .collect();

            let entry = NewServerConfigEntry {
                key: "jwt_secret".to_string(),
                value: secret.clone(),
            };
            diesel::insert_into(server_config::table)
                .values(&entry)
                .execute(conn)
                .context("Failed to insert JWT secret")?;

            info!("Generated new JWT secret");
            secret
        }
    };

    Ok(std::env::var("JWT_SECRET").unwrap_or(jwt_secret))
}

/// Seed the default admin user if no users exist.
pub fn seed_admin_user(conn: &mut PgConnection) -> anyhow::Result<()> {
    let user_count: i64 = users::table
        .count()
        .get_result(conn)
        .context("Failed to count users")?;

    if user_count == 0 {
        let password_hash = auth::hash_password("admin")
            .map_err(|e| anyhow::anyhow!("Failed to hash default password: {e}"))?;
        let admin = NewUser {
            tenant_id: crate::tenancy::DEFAULT_TENANT_ID.to_string(),
            username: "admin".to_string(),
            password_hash,
        };
        diesel::insert_into(users::table)
            .values(&admin)
            .execute(conn)
            .context("Failed to seed admin user")?;
        tracing::warn!(
            "Default admin user created (username: admin, password: admin). Change this immediately!"
        );
    }
    Ok(())
}

/// Ensure the root CA certificate exists in the database.
pub fn init_ca_certificate(conn: &mut PgConnection) -> anyhow::Result<()> {
    let ca_exists: i64 = ca_certificates::table
        .count()
        .get_result(conn)
        .context("Failed to count CA certificates")?;

    if ca_exists == 0 {
        let new_ca =
            cert_service::generate_ca_certificate().context("Failed to generate CA certificate")?;
        cert_repo::insert_ca_certificate(conn, &new_ca)
            .context("Failed to insert CA certificate")?;
        info!("Generated new root CA certificate");
    }
    Ok(())
}

/// Write CA and server TLS certificates to disk for Zenoh.
pub fn write_tls_certs(conn: &mut PgConnection, certs_dir: &str) -> anyhow::Result<()> {
    let certs_path = std::path::PathBuf::from(certs_dir);
    std::fs::create_dir_all(&certs_path).context("Failed to create certs directory")?;

    let ca = cert_repo::get_ca_certificate(conn)
        .context("Failed to read CA certificate")?
        .context("CA certificate must exist")?;

    std::fs::write(certs_path.join("ca.pem"), &ca.certificate_pem)
        .context("Failed to write CA cert to disk")?;

    let server_cert_path = certs_path.join("server.pem");
    let server_key_path = certs_path.join("server-key.pem");
    if !server_cert_path.exists() || !server_key_path.exists() {
        let (server_cert_pem, server_key_pem) = cert_service::generate_server_certificate(&ca)
            .context("Failed to generate server certificate")?;
        std::fs::write(&server_cert_path, &server_cert_pem)
            .context("Failed to write server cert to disk")?;
        std::fs::write(&server_key_path, &server_key_pem)
            .context("Failed to write server key to disk")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&server_key_path, std::fs::Permissions::from_mode(0o600))
                .context("Failed to set server key permissions")?;
        }

        info!("Generated server TLS certificate");
    }

    info!("TLS certificates at {}", certs_dir);
    Ok(())
}

/// Configure and open a Zenoh session.
pub async fn open_zenoh_session(
    tls_enabled: bool,
    tls_port: u16,
    certs_dir: &str,
) -> anyhow::Result<zenoh::Session> {
    let mut zenoh_config = zenoh::Config::default();
    let listen_scheme = if tls_enabled { "tls" } else { "tcp" };
    let listen_endpoint = format!("{listen_scheme}/0.0.0.0:{tls_port}");

    if tls_enabled {
        let certs_path =
            std::fs::canonicalize(certs_dir).context("Failed to resolve certs directory path")?;
        let ca_path = certs_path.join("ca.pem");
        let server_cert_path = certs_path.join("server.pem");
        let server_key_path = certs_path.join("server-key.pem");

        for path in [&ca_path, &server_cert_path, &server_key_path] {
            anyhow::ensure!(path.exists(), "Missing TLS file: {}", path.display());
        }

        zenoh_config
            .insert_json5("listen/endpoints", &format!("[\"{listen_endpoint}\"]"))
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh listen endpoints: {e}"))?;

        zenoh_config
            .insert_json5(
                "transport/link/tls/root_ca_certificate",
                &format!("\"{}\"", ca_path.display()),
            )
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh TLS root CA: {e}"))?;
        zenoh_config
            .insert_json5(
                "transport/link/tls/listen_certificate",
                &format!("\"{}\"", server_cert_path.display()),
            )
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh TLS server certificate: {e}"))?;
        zenoh_config
            .insert_json5(
                "transport/link/tls/listen_private_key",
                &format!("\"{}\"", server_key_path.display()),
            )
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh TLS server private key: {e}"))?;
        zenoh_config
            .insert_json5("transport/link/tls/enable_mtls", "true")
            .map_err(|e| anyhow::anyhow!("Failed to enable Zenoh mTLS: {e}"))?;

        zenoh_config
            .insert_json5("scouting/multicast/enabled", "false")
            .map_err(|e| anyhow::anyhow!("Failed to disable multicast scouting: {e}"))?;

        info!("Zenoh TLS configured: listening on {listen_endpoint} with mTLS");
    } else {
        zenoh_config
            .insert_json5("listen/endpoints", &format!("[\"{listen_endpoint}\"]"))
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh listen endpoints: {e}"))?;
        info!("Zenoh configured: listening on {listen_endpoint} (no TLS)");
    }

    zenoh::open(zenoh_config).await.map_err(|e| {
        let raw_error = e.to_string();
        debug!("Raw Zenoh open error: {raw_error}");
        anyhow::anyhow!(format_zenoh_open_error(
            &raw_error,
            &listen_endpoint,
            tls_port
        ))
    })
}

fn format_zenoh_open_error(raw_error: &str, listen_endpoint: &str, port: u16) -> String {
    let lower = raw_error.to_ascii_lowercase();
    if lower.contains("address already in use")
        || lower.contains("os error 48")
        || lower.contains("os error 98")
    {
        return format!(
            "Zenoh port {port} is already in use. Stop the existing Extrittio/Zenoh process, or start this instance with a different port, for example: ZENOH_TLS_PORT={} extrittio",
            port.saturating_add(1)
        );
    }

    format!(
        "Failed to open Zenoh session on {listen_endpoint}. Check Zenoh configuration, TLS settings, and port availability. Set RUST_LOG=extrittio_backend=debug for the raw Zenoh error."
    )
}

#[cfg(test)]
mod tests {
    use super::format_zenoh_open_error;

    #[test]
    fn zenoh_port_conflict_error_is_actionable() {
        let raw_error = "Can not create a new TCP listener bound to tcp/0.0.0.0:7447: [0.0.0.0:7447: Address already in use (os error 48) at /registry/src/tcp.rs:52.]";

        let message = format_zenoh_open_error(raw_error, "tcp/0.0.0.0:7447", 7447);

        assert!(message.contains("Zenoh port 7447 is already in use"));
        assert!(message.contains("ZENOH_TLS_PORT=7448 extrittio"));
        assert!(!message.contains("/registry/src"));
        assert!(!message.contains(".rs:"));
    }

    #[test]
    fn generic_zenoh_error_is_sanitized() {
        let message = format_zenoh_open_error(
            "some dependency detail at /registry/src/lib.rs:12",
            "tls/0.0.0.0:7447",
            7447,
        );

        assert!(message.contains("Failed to open Zenoh session on tls/0.0.0.0:7447"));
        assert!(!message.contains("/registry/src"));
        assert!(!message.contains(".rs:"));
    }
}
