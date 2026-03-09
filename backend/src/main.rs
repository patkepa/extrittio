use std::sync::Arc;

use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool};
use diesel::sqlite::SqliteConnection;
use diesel::RunQueryDsl;
use diesel_migrations::MigrationHarness;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

use extrittio_backend::config::AppConfig;
use extrittio_backend::state::AppState;
use extrittio_backend::{api, background, zenoh_handler, MIGRATIONS};

#[derive(Debug)]
struct SqlitePragmas;

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for SqlitePragmas {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        diesel::sql_query("PRAGMA foreign_keys = ON")
            .execute(conn)
            .map_err(|e| diesel::r2d2::Error::QueryError(e))?;
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

    // Run migrations
    {
        let mut conn = db_pool.get().expect("Failed to get DB connection for migrations");
        run_migrations(&mut conn);
    }

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

    // Set up CORS
    let origin = config.allowed_origin.clone();
    let cors = CorsLayer::new()
        .allow_origin(origin.parse::<axum::http::HeaderValue>().expect("Invalid CORS origin"))
        .allow_methods(Any)
        .allow_headers(Any);

    // Build Axum router
    let app = api::router().layer(cors).with_state(state);

    // Bind and serve
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    info!("Listening on {}", addr);

    axum::serve(listener, app).await.expect("Server error");
}
