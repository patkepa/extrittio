use std::collections::VecDeque;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use std::sync::Arc;

use crate::state::AppState;

/// Simple sliding-window rate limiter keyed by IP address.
/// Uses DashMap for lock-free concurrent access.
pub struct RateLimiter {
    state: DashMap<IpAddr, VecDeque<Instant>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            state: DashMap::new(),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Returns `true` if the request is allowed, `false` if rate-limited.
    pub fn check(&self, ip: &IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.state.entry(*ip).or_default();
        let timestamps = entry.value_mut();

        // Remove expired timestamps from the front (oldest first)
        while let Some(&front) = timestamps.front() {
            if now.duration_since(front) >= self.window {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        if timestamps.len() >= self.max_requests {
            false
        } else {
            timestamps.push_back(now);
            true
        }
    }

    /// Remove entries that haven't been seen since the window expired.
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.state.retain(|_, timestamps| {
            timestamps.back().is_some_and(|&t| now.duration_since(t) < self.window)
        });
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

/// Sliding-window rate limiter keyed by API key hash (string).
pub struct ApiKeyRateLimiter {
    state: DashMap<String, VecDeque<Instant>>,
    max_requests: usize,
    window: Duration,
}

impl ApiKeyRateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            state: DashMap::new(),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Returns `true` if the request is allowed, `false` if rate-limited.
    pub fn check(&self, key_hash: &str) -> bool {
        let now = Instant::now();
        let mut entry = self.state.entry(key_hash.to_string()).or_default();
        let timestamps = entry.value_mut();

        // Remove expired timestamps from the front (oldest first)
        while let Some(&front) = timestamps.front() {
            if now.duration_since(front) >= self.window {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        if timestamps.len() >= self.max_requests {
            false
        } else {
            timestamps.push_back(now);
            true
        }
    }
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
