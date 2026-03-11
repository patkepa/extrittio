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
        let certs_dir = std::env::var("EXTRITTIO_CERTS_DIR").unwrap_or_else(|_| "./certs".to_string());
        let certs_path = std::path::Path::new(&certs_dir);
        std::fs::create_dir_all(certs_path).expect("Failed to create certs directory");

        let ca = cert_repo::get_ca_certificate(&mut conn)
            .expect("Failed to read CA certificate")
            .expect("CA certificate must exist");

        std::fs::write(certs_path.join("ca.pem"), &ca.certificate_pem)
            .expect("Failed to write CA cert to disk");

        // Generate server cert for Zenoh TLS
        let (server_cert_pem, server_key_pem) =
            cert_service::generate_server_certificate(&ca)
                .expect("Failed to generate server certificate");
        std::fs::write(certs_path.join("server.pem"), &server_cert_pem)
            .expect("Failed to write server cert to disk");
        std::fs::write(certs_path.join("server-key.pem"), &server_key_pem)
            .expect("Failed to write server key to disk");

        info!("TLS certificates written to {}", certs_dir);

        // Allow override via environment variable
        std::env::var("JWT_SECRET").unwrap_or(jwt_secret)
    };

    // Open zenoh session
    let zenoh_config = zenoh::Config::default();

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

    // Bind and serve
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    info!("Listening on {}", addr);

    axum::serve(listener, app).await.expect("Server error");
}
