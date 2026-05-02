use std::sync::Arc;

use anyhow::Context;
use axum::middleware as axum_middleware;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

use crate::config::AppConfig;
use crate::middleware::auth_middleware;
use crate::state::AppState;
use crate::{api, rate_limit, services};

/// Build and serve the HTTP API.
pub async fn serve(config: &AppConfig, state: Arc<AppState>) -> anyhow::Result<()> {
    let origin = config.allowed_origin.clone();
    let cors = CorsLayer::new()
        .allow_origin(
            origin
                .parse::<axum::http::HeaderValue>()
                .context(format!("Invalid CORS origin: {origin}"))?,
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
        .context("Invalid listen address")?;
    let socket = socket2::Socket::new(
        socket2::Domain::for_address(addr),
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )
    .context("Failed to create socket")?;
    socket
        .set_reuse_address(true)
        .context("Failed to set SO_REUSEADDR")?;
    socket
        .set_nodelay(true)
        .context("Failed to set TCP_NODELAY")?;
    socket.bind(&addr.into()).context("Failed to bind socket")?;
    socket.listen(1024).context("Failed to listen")?;
    socket
        .set_nonblocking(true)
        .context("Failed to set non-blocking")?;
    let listener = tokio::net::TcpListener::from_std(socket.into())
        .context("Failed to create tokio TcpListener")?;
    info!("Listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server error")?;

    info!("Server shut down gracefully");
    Ok(())
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
