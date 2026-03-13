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
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing::info;
use tracing_subscriber::EnvFilter;

use extrittio_backend::config::AppConfig;
use extrittio_backend::init;
use extrittio_backend::middleware::auth_middleware;
use extrittio_backend::rate_limit::{self, ApiKeyRateLimiter, RateLimiter};
use extrittio_backend::state::{AppState, MetricsAccumulator, ZenohMetrics};
use extrittio_backend::services;
use extrittio_backend::{api, background, zenoh_handler};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("extrittio_backend=info")),
        )
        .init();

    let config = AppConfig::from_env();
    info!("Starting extrittio-backend on port {}", config.port);

    // Database
    let db_pool = init::create_db_pool(&config.database_url, config.db_pool_size);
    info!("DB connection pool: max_size={}", config.db_pool_size);

    let jwt_secret = {
        let mut conn = db_pool
            .get()
            .expect("Failed to get DB connection for initialization");
        init::run_migrations(&mut conn);
        let secret = init::init_jwt_secret(&mut conn);
        init::seed_admin_user(&mut conn);
        init::init_ca_certificate(&mut conn);
        init::write_tls_certs(&mut conn, &config.certs_dir);
        secret
    };

    // Zenoh
    let zenoh_session = Arc::new(
        init::open_zenoh_session(
            config.zenoh_tls_enabled,
            config.zenoh_tls_port,
            &config.certs_dir,
        )
        .await,
    );
    info!("Zenoh session opened");

    // Zenoh metrics
    let zenoh_metrics = Arc::new(ZenohMetrics::new());

    // Application state
    let state = Arc::new(AppState {
        db_pool: db_pool.clone(),
        zenoh_session: zenoh_session.clone(),
        jwt_secret,
        api_rate_limiter: RateLimiter::new(100, 60),
        login_rate_limiter: RateLimiter::new(5, 60),
        ci_rate_limiter: ApiKeyRateLimiter::new(60, 60),
        metrics_accumulator: MetricsAccumulator::new(),
        zenoh_metrics: zenoh_metrics.clone(),
    });

    // Background tasks
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

    let checker_pool = db_pool.clone();
    let offline_timeout = config.offline_timeout_secs;
    tokio::spawn(async move {
        background::run_offline_checker(checker_pool, offline_timeout).await;
    });

    let cmd_timeout_pool = db_pool.clone();
    let cmd_timeout = config.command_timeout_secs;
    tokio::spawn(async move {
        background::run_command_timeout_checker(cmd_timeout_pool, cmd_timeout).await;
    });

    // HTTP server
    let origin = config.allowed_origin.clone();
    let cors = CorsLayer::new()
        .allow_origin(
            origin
                .parse::<axum::http::HeaderValue>()
                .expect("Invalid CORS origin"),
        )
        .allow_methods(Any)
        .allow_headers(Any);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

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
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            services::metrics_middleware::metrics_middleware,
        ))
        .layer(cors)
        .with_state(state);

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
