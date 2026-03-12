#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::doc_markdown,
    clippy::similar_names,
    clippy::too_many_lines
)]

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use std::sync::Arc;

use axum::middleware as axum_middleware;
use diesel::RunQueryDsl;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing::info;
use tracing_subscriber::EnvFilter;

use extrittio_backend::auth::hash_password;
use extrittio_backend::config::AppConfig;
use extrittio_backend::db::models::{NewServerConfigEntry, NewUser, ServerConfigEntry};
use extrittio_backend::db::schema::{ca_certificates, server_config, users};
use extrittio_backend::middleware::auth_middleware;
use extrittio_backend::rate_limit::{self, RateLimiter};
use extrittio_backend::repositories::cert_repo;
use extrittio_backend::services::cert_service;
use extrittio_backend::state::AppState;
use extrittio_backend::{MIGRATIONS, api, background, zenoh_handler};

#[derive(Debug)]
struct SqlitePragmas;

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for SqlitePragmas {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        // Enforce referential integrity
        diesel::sql_query("PRAGMA foreign_keys = ON")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        // Return SQLITE_BUSY immediately instead of blocking (Tokio-friendly)
        diesel::sql_query("PRAGMA busy_timeout = 5000")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        // Keep temp tables in memory instead of disk
        diesel::sql_query("PRAGMA temp_store = MEMORY")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        // Increase page cache to ~32MB (8192 pages × 4KB)
        diesel::sql_query("PRAGMA cache_size = -32000")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        // Memory-mapped I/O — let the kernel manage page caching (256MB)
        diesel::sql_query("PRAGMA mmap_size = 268435456")
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

    // NORMAL sync is safe with WAL — avoids fsync on every commit
    diesel::sql_query("PRAGMA synchronous = NORMAL")
        .execute(conn)
        .expect("Failed to set synchronous mode");

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
        .max_size(config.db_pool_size)
        .connection_customizer(Box::new(SqlitePragmas))
        .build(manager)
        .expect("Failed to create database connection pool");
    info!("DB connection pool: max_size={}", config.db_pool_size);

    // Run migrations, initialize JWT secret, and seed admin user
    let jwt_secret = {
        let mut conn = db_pool
            .get()
            .expect("Failed to get DB connection for migrations");
        run_migrations(&mut conn);

        // Initialize JWT secret if not present
        let jwt_secret = {
            let existing: Option<ServerConfigEntry> = server_config::table
                .find("jwt_secret")
                .select(ServerConfigEntry::as_select())
                .first(&mut conn)
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
                    .execute(&mut conn)
                    .expect("Failed to insert JWT secret");

                info!("Generated new JWT secret");
                secret
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
            tracing::warn!(
                "Default admin user created (username: admin, password: admin). Change this immediately!"
            );
        }

        // Initialize CA certificate if not present
        let ca_exists: i64 = ca_certificates::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count CA certificates");

        if ca_exists == 0 {
            let new_ca =
                cert_service::generate_ca_certificate().expect("Failed to generate CA certificate");
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
            let (server_cert_pem, server_key_pem) = cert_service::generate_server_certificate(&ca)
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
    } else {
        // Listen on plain TCP so clients can connect with --connect tcp/host:port
        let listen_endpoint = format!("tcp/0.0.0.0:{}", config.zenoh_tls_port);
        zenoh_config
            .insert_json5("listen/endpoints", &format!("[\"{listen_endpoint}\"]"))
            .expect("Failed to set Zenoh listen endpoints");
        info!("Zenoh configured: listening on {listen_endpoint} (no TLS)");
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
        api_rate_limiter: RateLimiter::new(100, 60), // 100 req/min per IP
        login_rate_limiter: RateLimiter::new(5, 60), // 5 req/min per IP
    });

    // Spawn zenoh subscriber task
    let subscriber_pool = db_pool.clone();
    let subscriber_session = zenoh_session.clone();
    tokio::spawn(async move {
        if let Err(e) =
            zenoh_handler::subscriber::run_subscriber(subscriber_session, subscriber_pool).await
        {
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
        .allow_origin(
            origin
                .parse::<axum::http::HeaderValue>()
                .expect("Invalid CORS origin"),
        )
        .allow_methods(Any)
        .allow_headers(Any);

    // Request tracing
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    // Build Axum router
    let app = api::router(config.max_firmware_size_bytes)
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_middleware,
        ))
        .layer(trace_layer)
        .layer(cors)
        .with_state(state);

    // Bind with tuned socket options for high-connection IoT workloads
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", config.port)
        .parse()
        .expect("Invalid listen address");
    let socket = socket2::Socket::new(
        socket2::Domain::for_address(addr),
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )
    .expect("Failed to create socket");
    socket
        .set_reuse_address(true)
        .expect("Failed to set SO_REUSEADDR");
    socket.set_nodelay(true).expect("Failed to set TCP_NODELAY");
    // Increase listen backlog for burst connections from many devices
    socket.bind(&addr.into()).expect("Failed to bind socket");
    socket.listen(1024).expect("Failed to listen");
    socket
        .set_nonblocking(true)
        .expect("Failed to set non-blocking");
    let listener = tokio::net::TcpListener::from_std(socket.into())
        .expect("Failed to create tokio TcpListener");
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
