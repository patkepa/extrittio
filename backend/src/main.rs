use std::sync::Arc;

use axum::middleware as axum_middleware;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool};
use diesel::sqlite::SqliteConnection;
use diesel::RunQueryDsl;
use diesel_migrations::MigrationHarness;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

use extrittio_backend::auth::hash_password;
use extrittio_backend::config::AppConfig;
use extrittio_backend::db::models::{NewServerConfigEntry, NewUser, ServerConfigEntry};
use extrittio_backend::db::schema::{ca_certificates, server_config, users};
use extrittio_backend::middleware::auth_middleware;
use extrittio_backend::repositories::cert_repo;
use extrittio_backend::services::cert_service;
use extrittio_backend::state::AppState;
use extrittio_backend::{api, background, zenoh_handler, MIGRATIONS};

#[derive(Debug)]
struct SqlitePragmas;

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for SqlitePragmas {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        diesel::sql_query("PRAGMA foreign_keys = ON")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        Ok(())
    }
}

fn run_migrations(conn: &mut SqliteConnection) {
    // Enable WAL journal mode for SQLite (per-database, only needs to run once)
    diesel::sql_query("PRAGMA journal_mode = WAL")
        .execute(conn)
        .expect("Failed to set WAL journal mode");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");

    info!("Database migrations completed successfully");
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("extrittio_backend=info")),
        )
        .init();

    // Load configuration
    let config = AppConfig::from_env();
    info!("Starting extrittio-backend on port {}", config.port);

    // Create database connection pool
    let manager = ConnectionManager::<SqliteConnection>::new(&config.database_url);
    let db_pool = Pool::builder()
        .max_size(4)
        .connection_customizer(Box::new(SqlitePragmas))
        .build(manager)
        .expect("Failed to create database connection pool");

    // Run migrations, initialize JWT secret, and seed admin user
    let jwt_secret = {
        let mut conn = db_pool.get().expect("Failed to get DB connection for migrations");
        run_migrations(&mut conn);

        // Initialize JWT secret if not present
        let jwt_secret = {
            let existing: Option<ServerConfigEntry> = server_config::table
                .find("jwt_secret")
                .select(ServerConfigEntry::as_select())
                .first(&mut conn)
                .optional()
                .expect("Failed to query server_config");

            match existing {
                Some(entry) => entry.value,
                None => {
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
                        .execute(&mut conn)
                        .expect("Failed to insert JWT secret");

                    info!("Generated new JWT secret");
                    secret
                }
            }
        };

        // Seed default admin user if no users exist
        let user_count: i64 = users::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count users");

        if user_count == 0 {
            let password_hash = hash_password("admin").expect("Failed to hash default password");
            let admin = NewUser {
                username: "admin".to_string(),
                password_hash,
            };
            diesel::insert_into(users::table)
                .values(&admin)
                .execute(&mut conn)
                .expect("Failed to seed admin user");
            tracing::warn!("Default admin user created (username: admin, password: admin). Change this immediately!");
        }

        // Initialize CA certificate if not present
        let ca_exists: i64 = ca_certificates::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count CA certificates");

        if ca_exists == 0 {
            let new_ca = cert_service::generate_ca_certificate()
                .expect("Failed to generate CA certificate");
            cert_repo::insert_ca_certificate(&mut conn, &new_ca)
                .expect("Failed to insert CA certificate");
            info!("Generated new root CA certificate");
        }

        // Write CA and server certs to disk for Zenoh TLS
        let certs_path = std::path::PathBuf::from(&config.certs_dir);
        std::fs::create_dir_all(&certs_path).expect("Failed to create certs directory");

        let ca = cert_repo::get_ca_certificate(&mut conn)
            .expect("Failed to read CA certificate")
            .expect("CA certificate must exist");

        std::fs::write(certs_path.join("ca.pem"), &ca.certificate_pem)
            .expect("Failed to write CA cert to disk");

        // Only generate server cert if it doesn't already exist on disk
        let server_cert_path = certs_path.join("server.pem");
        let server_key_path = certs_path.join("server-key.pem");
        if !server_cert_path.exists() || !server_key_path.exists() {
            let (server_cert_pem, server_key_pem) =
                cert_service::generate_server_certificate(&ca)
                    .expect("Failed to generate server certificate");
            std::fs::write(&server_cert_path, &server_cert_pem)
                .expect("Failed to write server cert to disk");
            std::fs::write(&server_key_path, &server_key_pem)
                .expect("Failed to write server key to disk");

            // Restrict private key file permissions (owner read-only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&server_key_path, std::fs::Permissions::from_mode(0o600))
                    .expect("Failed to set server key permissions");
            }

            info!("Generated server TLS certificate");
        }

        info!("TLS certificates at {}", config.certs_dir);

        // Allow override via environment variable
        std::env::var("JWT_SECRET").unwrap_or(jwt_secret)
    };

    // Open zenoh session (with optional mTLS)
    let mut zenoh_config = zenoh::Config::default();

    if config.zenoh_tls_enabled {
        let certs_path = std::fs::canonicalize(&config.certs_dir)
            .expect("Failed to resolve certs directory path");
        let ca_path = certs_path.join("ca.pem");
        let server_cert_path = certs_path.join("server.pem");
        let server_key_path = certs_path.join("server-key.pem");

        // Verify all required cert files exist
        for path in [&ca_path, &server_cert_path, &server_key_path] {
            assert!(path.exists(), "Missing TLS file: {}", path.display());
        }

        let listen_endpoint = format!("tls/0.0.0.0:{}", config.zenoh_tls_port);
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

        // Disable multicast scouting when using TLS (devices connect directly)
        zenoh_config
            .insert_json5("scouting/multicast/enabled", "false")
            .expect("Failed to disable multicast scouting");

        info!("Zenoh TLS configured: listening on {listen_endpoint} with mTLS");
    }

    let zenoh_session = zenoh::open(zenoh_config)
        .await
        .expect("Failed to open zenoh session");
    let zenoh_session = Arc::new(zenoh_session);

    info!("Zenoh session opened");

    // Build shared application state
    let state = Arc::new(AppState {
        db_pool: db_pool.clone(),
        zenoh_session: zenoh_session.clone(),
        jwt_secret,
    });

    // Spawn zenoh subscriber task
    let subscriber_pool = db_pool.clone();
    let subscriber_session = zenoh_session.clone();
    tokio::spawn(async move {
        if let Err(e) = zenoh_handler::subscriber::run_subscriber(subscriber_session, subscriber_pool).await {
            tracing::error!("Zenoh subscriber failed: {}. Shutting down.", e);
            std::process::exit(1);
        }
    });

    // Spawn offline checker task
    let checker_pool = db_pool.clone();
    let offline_timeout = config.offline_timeout_secs;
    tokio::spawn(async move {
        background::run_offline_checker(checker_pool, offline_timeout).await;
    });

    // Spawn command timeout checker task
    let cmd_timeout_pool = db_pool.clone();
    let cmd_timeout = config.command_timeout_secs;
    tokio::spawn(async move {
        background::run_command_timeout_checker(cmd_timeout_pool, cmd_timeout).await;
    });

    // Set up CORS
    let origin = config.allowed_origin.clone();
    let cors = CorsLayer::new()
        .allow_origin(origin.parse::<axum::http::HeaderValue>().expect("Invalid CORS origin"))
        .allow_methods(Any)
        .allow_headers(Any);

    // Build Axum router
    let app = api::router()
        .layer(axum_middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(cors)
        .with_state(state);

    // Bind and serve with graceful shutdown
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    info!("Listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");

    info!("Server shut down gracefully");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => info!("Received Ctrl+C, starting graceful shutdown"),
        () = terminate => info!("Received SIGTERM, starting graceful shutdown"),
    }
}
