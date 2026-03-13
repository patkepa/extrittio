use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// Metrics middleware — placed outermost (after CORS, before auth) to capture all requests.
pub async fn metrics_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let response = next.run(request).await;
    let latency_micros = start.elapsed().as_micros() as u64;
    let is_error = response.status().is_client_error() || response.status().is_server_error();
    state.metrics_accumulator.record(latency_micros, is_error);
    response
}
