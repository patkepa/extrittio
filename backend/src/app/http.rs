use std::{
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use axum::{
    Extension, Router,
    body::Body,
    http::{StatusCode, Uri, header},
    middleware as axum_middleware,
    response::{IntoResponse, Response},
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info, warn};

use crate::config::AppConfig;
use crate::middleware::auth_middleware;
use crate::state::AppState;
use crate::{api, rate_limit, services};

/// Build and serve the HTTP API.
pub async fn serve(config: &AppConfig, state: Arc<AppState>) -> anyhow::Result<()> {
    let origin = config.allowed_origin.clone();
    anyhow::ensure!(
        origin != "*",
        "CORS_ORIGIN='*' is not allowed; set it to the public web origin"
    );
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

    let app = api::router(config.max_firmware_size_bytes, config.enable_api_docs)
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
    let app = attach_frontend_ui(app, config);

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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("Server error")?;

    info!("Server shut down gracefully");
    Ok(())
}

#[derive(Clone)]
struct FrontendUi {
    root: PathBuf,
    index: PathBuf,
}

fn attach_frontend_ui(app: Router, config: &AppConfig) -> Router {
    if !config.serve_ui {
        info!("Frontend UI serving disabled");
        return app;
    }

    let Some(root) = resolve_ui_dir(config) else {
        warn!(
            "Frontend UI build not found; serving API only. Run `npm run build` in frontend/ or set EXTRITTIO_UI_DIR."
        );
        return app;
    };

    let index = root.join("index.html");
    info!("Serving frontend UI from {}", root.display());

    app.fallback(spa_fallback)
        .layer(Extension(Arc::new(FrontendUi { root, index })))
}

fn resolve_ui_dir(config: &AppConfig) -> Option<PathBuf> {
    if let Some(root) = &config.ui_dir {
        return validate_ui_dir(root);
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("frontend/dist"));
        candidates.push(cwd.join("dist"));
        candidates.push(cwd.join("../frontend/dist"));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        candidates.push(exe_dir.join("frontend/dist"));
        candidates.push(exe_dir.join("../frontend/dist"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist"));

    candidates
        .into_iter()
        .find_map(|candidate| validate_ui_dir(&candidate))
}

fn validate_ui_dir(path: &Path) -> Option<PathBuf> {
    let index = path.join("index.html");
    if index.is_file() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

async fn spa_fallback(
    uri: Uri,
    Extension(ui): Extension<Arc<FrontendUi>>,
) -> Result<Response, StatusCode> {
    let path = uri.path();
    if is_backend_reserved_path(path) {
        return Err(StatusCode::NOT_FOUND);
    }

    let Some(relative_path) = sanitized_relative_path(path) else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let requested_path = ui.root.join(&relative_path);
    let file_path = if tokio::fs::metadata(&requested_path)
        .await
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
    {
        requested_path
    } else if relative_path.extension().is_none() {
        ui.index.clone()
    } else {
        return Err(StatusCode::NOT_FOUND);
    };

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, content_type(&file_path))
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

fn is_backend_reserved_path(path: &str) -> bool {
    path == "/api"
        || path.starts_with("/api/")
        || path == "/health"
        || path == "/ready"
        || path == "/api-docs"
        || path.starts_with("/api-docs/")
        || path == "/swagger-ui"
        || path.starts_with("/swagger-ui/")
}

fn sanitized_relative_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return Some(PathBuf::from("index.html"));
    }

    let mut result = PathBuf::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(result)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webmanifest") => "application/manifest+json",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
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
