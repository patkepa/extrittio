use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;

use crate::state::AppState;

/// Simple sliding-window rate limiter keyed by IP address.
pub struct RateLimiter {
    state: Mutex<HashMap<IpAddr, Vec<Instant>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Returns `true` if the request is allowed, `false` if rate-limited.
    pub fn check(&self, ip: &IpAddr) -> bool {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap();
        let entry = state.entry(*ip).or_default();

        // Remove expired timestamps
        entry.retain(|&t| now.duration_since(t) < self.window);

        if entry.len() >= self.max_requests {
            false
        } else {
            entry.push(now);
            true
        }
    }
}

/// Extract the client IP from X-Forwarded-For (first hop) or fall back to localhost.
fn extract_client_ip(request: &Request) -> IpAddr {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Skip rate limiting for health probes
    if path == "/health" || path == "/ready" {
        return next.run(request).await;
    }

    let ip = extract_client_ip(&request);

    // Stricter limit for login, general limit for everything else
    let limiter = if path == "/api/v1/auth/login" {
        &state.login_rate_limiter
    } else {
        &state.api_rate_limiter
    };

    if limiter.check(&ip) {
        next.run(request).await
    } else {
        let body = serde_json::json!({ "error": "Too many requests" });
        (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response()
    }
}
