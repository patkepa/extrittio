use diesel::RunQueryDsl;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use tracing::info;

use crate::db::models::{NewServerConfigEntry, NewUser, ServerConfigEntry};
use crate::db::schema::{ca_certificates, server_config, users};
use crate::repositories::cert_repo;
use crate::services::cert_service;
use crate::state::DbPool;
use crate::{MIGRATIONS, auth};

#[derive(Debug)]
struct SqlitePragmas;

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for SqlitePragmas {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        diesel::sql_query("PRAGMA foreign_keys = ON")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        diesel::sql_query("PRAGMA busy_timeout = 5000")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        diesel::sql_query("PRAGMA temp_store = MEMORY")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        diesel::sql_query("PRAGMA cache_size = -32000")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        diesel::sql_query("PRAGMA mmap_size = 268435456")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        Ok(())
    }
}

/// Create the SQLite connection pool with tuned pragmas.
pub fn create_db_pool(database_url: &str, pool_size: u32) -> DbPool {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .max_size(pool_size)
        .connection_customizer(Box::new(SqlitePragmas))
        .build(manager)
        .expect("Failed to create database connection pool")
}

/// Run pending migrations and set WAL journal mode.
pub fn run_migrations(conn: &mut SqliteConnection) {
    diesel::sql_query("PRAGMA journal_mode = WAL")
        .execute(conn)
        .expect("Failed to set WAL journal mode");

    diesel::sql_query("PRAGMA synchronous = NORMAL")
        .execute(conn)
        .expect("Failed to set synchronous mode");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");

    info!("Database migrations completed successfully");
}

/// Initialize JWT secret from the database, or generate one if not present.
/// Falls back to the `JWT_SECRET` environment variable if set.
pub fn init_jwt_secret(conn: &mut SqliteConnection) -> String {
    let jwt_secret = {
        let existing: Option<ServerConfigEntry> = server_config::table
            .find("jwt_secret")
            .select(ServerConfigEntry::as_select())
            .first(conn)
            .optional()
            .expect("Failed to query server_config");

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
                .expect("Failed to insert JWT secret");

            info!("Generated new JWT secret");
            secret
        }
    };

    std::env::var("JWT_SECRET").unwrap_or(jwt_secret)
}

/// Seed the default admin user if no users exist.
pub fn seed_admin_user(conn: &mut SqliteConnection) {
    let user_count: i64 = users::table
        .count()
        .get_result(conn)
        .expect("Failed to count users");

    if user_count == 0 {
        let password_hash = auth::hash_password("admin").expect("Failed to hash default password");
        let admin = NewUser {
            username: "admin".to_string(),
            password_hash,
        };
        diesel::insert_into(users::table)
            .values(&admin)
            .execute(conn)
            .expect("Failed to seed admin user");
        tracing::warn!(
            "Default admin user created (username: admin, password: admin). Change this immediately!"
        );
    }
}

/// Ensure the root CA certificate exists in the database.
pub fn init_ca_certificate(conn: &mut SqliteConnection) {
    let ca_exists: i64 = ca_certificates::table
        .count()
        .get_result(conn)
        .expect("Failed to count CA certificates");

    if ca_exists == 0 {
        let new_ca =
            cert_service::generate_ca_certificate().expect("Failed to generate CA certificate");
        cert_repo::insert_ca_certificate(conn, &new_ca)
            .expect("Failed to insert CA certificate");
        info!("Generated new root CA certificate");
    }
}

/// Write CA and server TLS certificates to disk for Zenoh.
pub fn write_tls_certs(conn: &mut SqliteConnection, certs_dir: &str) {
    let certs_path = std::path::PathBuf::from(certs_dir);
    std::fs::create_dir_all(&certs_path).expect("Failed to create certs directory");

    let ca = cert_repo::get_ca_certificate(conn)
        .expect("Failed to read CA certificate")
        .expect("CA certificate must exist");

    std::fs::write(certs_path.join("ca.pem"), &ca.certificate_pem)
        .expect("Failed to write CA cert to disk");

    let server_cert_path = certs_path.join("server.pem");
    let server_key_path = certs_path.join("server-key.pem");
    if !server_cert_path.exists() || !server_key_path.exists() {
        let (server_cert_pem, server_key_pem) = cert_service::generate_server_certificate(&ca)
            .expect("Failed to generate server certificate");
        std::fs::write(&server_cert_path, &server_cert_pem)
            .expect("Failed to write server cert to disk");
        std::fs::write(&server_key_path, &server_key_pem)
            .expect("Failed to write server key to disk");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&server_key_path, std::fs::Permissions::from_mode(0o600))
                .expect("Failed to set server key permissions");
        }

        info!("Generated server TLS certificate");
    }

    info!("TLS certificates at {}", certs_dir);
}

/// Configure and open a Zenoh session.
pub async fn open_zenoh_session(
    tls_enabled: bool,
    tls_port: u16,
    certs_dir: &str,
) -> zenoh::Session {
    let mut zenoh_config = zenoh::Config::default();

    if tls_enabled {
        let certs_path = std::fs::canonicalize(certs_dir)
            .expect("Failed to resolve certs directory path");
        let ca_path = certs_path.join("ca.pem");
        let server_cert_path = certs_path.join("server.pem");
        let server_key_path = certs_path.join("server-key.pem");

        for path in [&ca_path, &server_cert_path, &server_key_path] {
            assert!(path.exists(), "Missing TLS file: {}", path.display());
        }

        let listen_endpoint = format!("tls/0.0.0.0:{tls_port}");
        zenoh_config
            .insert_json5("listen/endpoints", &format!("[\"{listen_endpoint}\"]"))
            .expect("Failed to set Zenoh listen endpoints");

        zenoh_config
            .insert_json5(
                "transport/link/tls/root_ca_certificate",
                &format!("\"{}\"", ca_path.display()),
            )
            .expect("Failed to set Zenoh TLS root CA");
        zenoh_config
            .insert_json5(
                "transport/link/tls/listen_certificate",
                &format!("\"{}\"", server_cert_path.display()),
            )
            .expect("Failed to set Zenoh TLS server certificate");
        zenoh_config
            .insert_json5(
                "transport/link/tls/listen_private_key",
                &format!("\"{}\"", server_key_path.display()),
            )
            .expect("Failed to set Zenoh TLS server private key");
        zenoh_config
            .insert_json5("transport/link/tls/enable_mtls", "true")
            .expect("Failed to enable Zenoh mTLS");

        zenoh_config
            .insert_json5("scouting/multicast/enabled", "false")
            .expect("Failed to disable multicast scouting");

        info!("Zenoh TLS configured: listening on {listen_endpoint} with mTLS");
    } else {
        let listen_endpoint = format!("tcp/0.0.0.0:{tls_port}");
        zenoh_config
            .insert_json5("listen/endpoints", &format!("[\"{listen_endpoint}\"]"))
            .expect("Failed to set Zenoh listen endpoints");
        info!("Zenoh configured: listening on {listen_endpoint} (no TLS)");
    }

    zenoh::open(zenoh_config)
        .await
        .expect("Failed to open zenoh session")
}
