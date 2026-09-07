use std::{
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use axum::{
    Extension, Router,
    body::{Body, Bytes},
    extract::Request,
    http::{HeaderName, HeaderValue, StatusCode, Uri, header},
    middleware as axum_middleware,
    middleware::Next,
    response::{IntoResponse, Response},
};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::{Level, info, warn};

use crate::config::AppConfig;
use crate::middleware::auth_middleware;
use crate::state::AppState;
use crate::{api, rate_limit, services};

/// Build and serve the HTTP API.
pub async fn serve(
    config: &AppConfig,
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
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
        .make_span_with(|request: &Request| tracing::info_span!("http",
            method = %request.method(), uri = %crate::middleware::redacted_request_uri(request.uri())))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    let app = api::router(config.max_firmware_size_bytes, config.enable_api_docs)
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::audit_middleware,
        ))
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
    let app = attach_frontend_ui(app, config)
        .layer(axum_middleware::from_fn(security_headers))
        .layer(axum_middleware::from_fn(
            crate::middleware::request_id_middleware,
        ));

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
        .set_tcp_nodelay(true)
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
    .with_graceful_shutdown(shutdown_signal(shutdown))
    .await
    .context("Server error")?;

    info!("Server shut down gracefully");
    Ok(())
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );

    let is_html = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    if is_html {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; connect-src 'self'; img-src 'self' data: https://*.tile.openstreetmap.org; style-src 'self' 'unsafe-inline'; script-src 'self'; font-src 'self' data:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'",
            ),
        );
    }

    response
}

#[derive(Clone)]
struct FrontendUi {
    root: Option<PathBuf>,
    index_html: Bytes,
}

#[cfg(feature = "embedded-ui")]
static EMBEDDED_UI: include_dir::Dir<'static> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../apps/frontend/dist");

fn attach_frontend_ui(app: Router, config: &AppConfig) -> Router {
    if !config.serve_ui {
        info!("Frontend UI serving disabled");
        return app;
    }

    let root = resolve_ui_dir(config);
    #[cfg(feature = "embedded-ui")]
    if root.is_none()
        && let Some(index) = EMBEDDED_UI.get_file("index.html")
    {
        info!("Serving frontend UI embedded in the executable");
        return app
            .fallback(spa_fallback)
            .layer(Extension(Arc::new(FrontendUi {
                root: None,
                index_html: Bytes::from_static(index.contents()),
            })));
    }
    let Some(root) = root else {
        warn!(
            "Frontend UI build not found; serving API only. Run `npm run build` in apps/frontend/ or set EXTRITTIO_UI_DIR."
        );
        return app;
    };

    let index = root.join("index.html");
    let index_html = match std::fs::read(&index) {
        Ok(bytes) => Bytes::from(bytes),
        Err(e) => {
            warn!("Frontend UI index could not be read: {}", e);
            return app;
        }
    };
    info!("Serving frontend UI from {}", root.display());

    app.fallback(spa_fallback)
        .layer(Extension(Arc::new(FrontendUi {
            root: Some(root),
            index_html,
        })))
}

fn resolve_ui_dir(config: &AppConfig) -> Option<PathBuf> {
    if let Some(root) = &config.ui_dir {
        return validate_ui_dir(root);
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("apps/frontend/dist"));
        candidates.push(cwd.join("dist"));
        candidates.push(cwd.join("../frontend/dist"));
        candidates.push(cwd.join("../../apps/frontend/dist"));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        candidates.push(exe_dir.join("../../apps/frontend/dist"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/frontend/dist"));

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

    if is_index_path(&relative_path) {
        return Ok(static_response(
            Path::new("index.html"),
            Body::from(ui.index_html.clone()),
            HeaderValue::from_static("no-cache"),
        ));
    }

    #[cfg(feature = "embedded-ui")]
    if ui.root.is_none() {
        let key = relative_path.to_string_lossy();
        if let Some(file) = EMBEDDED_UI.get_file(key.as_ref()) {
            return Ok(static_response(
                &relative_path,
                Body::from(Bytes::from_static(file.contents())),
                cache_control_for_path(&relative_path),
            ));
        }
        if relative_path.extension().is_none() {
            return Ok(static_response(
                Path::new("index.html"),
                Body::from(ui.index_html.clone()),
                HeaderValue::from_static("no-cache"),
            ));
        }
        return Err(StatusCode::NOT_FOUND);
    }

    let root = ui.root.as_ref().ok_or(StatusCode::NOT_FOUND)?;
    let requested_path = root.join(&relative_path);
    let file_path = if tokio::fs::metadata(&requested_path)
        .await
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
    {
        requested_path
    } else if relative_path.extension().is_none() {
        return Ok(static_response(
            Path::new("index.html"),
            Body::from(ui.index_html.clone()),
            HeaderValue::from_static("no-cache"),
        ));
    } else {
        return Err(StatusCode::NOT_FOUND);
    };

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(static_response(
        &file_path,
        Body::from(bytes),
        cache_control_for_path(&relative_path),
    ))
}

fn is_index_path(path: &Path) -> bool {
    path == Path::new("index.html")
}

fn cache_control_for_path(path: &Path) -> HeaderValue {
    if path.starts_with("assets") {
        HeaderValue::from_static("public, max-age=31536000, immutable")
    } else {
        HeaderValue::from_static("no-cache")
    }
}

fn static_response(file_path: &Path, body: Body, cache_control: HeaderValue) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type(file_path))
        .header(header::CACHE_CONTROL, cache_control)
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
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

async fn shutdown_signal(shutdown: CancellationToken) {
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
        () = shutdown.cancelled() => {
            warn!("Worker supervisor requested HTTP shutdown");
            return;
        }
    }
    shutdown.cancel();
}
