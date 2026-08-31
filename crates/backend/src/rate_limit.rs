use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use std::sync::Arc;

use crate::auth::context::{MappedUserClaims, RequestContext, map_validated_user_claims};
use crate::state::AppState;

const MAX_RATE_LIMIT_KEYS: usize = 100_000;

#[derive(Debug, Clone)]
pub struct TrustedProxy {
    network: IpAddr,
    prefix_len: u8,
}

impl TrustedProxy {
    pub fn parse(value: &str) -> Option<Self> {
        let (addr, prefix_len) = match value.split_once('/') {
            Some((addr, prefix)) => {
                let addr: IpAddr = addr.trim().parse().ok()?;
                let prefix_len: u8 = prefix.trim().parse().ok()?;
                (addr, prefix_len)
            }
            None => {
                let addr: IpAddr = value.trim().parse().ok()?;
                let prefix_len = match addr {
                    IpAddr::V4(_) => 32,
                    IpAddr::V6(_) => 128,
                };
                (addr, prefix_len)
            }
        };

        let max_prefix = match addr {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        (prefix_len <= max_prefix).then_some(Self {
            network: addr,
            prefix_len,
        })
    }

    fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => cidr_contains_v4(network, ip, self.prefix_len),
            (IpAddr::V6(network), IpAddr::V6(ip)) => cidr_contains_v6(network, ip, self.prefix_len),
            _ => false,
        }
    }
}

pub fn parse_trusted_proxies(values: &[String]) -> Vec<TrustedProxy> {
    let mut proxies = vec![
        TrustedProxy {
            network: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 0)),
            prefix_len: 8,
        },
        TrustedProxy {
            network: IpAddr::V6(Ipv6Addr::LOCALHOST),
            prefix_len: 128,
        },
    ];

    for value in values {
        match TrustedProxy::parse(value) {
            Some(proxy) => proxies.push(proxy),
            None => tracing::warn!("Ignoring invalid trusted proxy entry `{value}`"),
        }
    }

    proxies
}

fn cidr_contains_v4(network: Ipv4Addr, ip: Ipv4Addr, prefix_len: u8) -> bool {
    let mask = if prefix_len == 0 {
        0
    } else {
        u32::MAX << (32 - prefix_len)
    };
    (u32::from(network) & mask) == (u32::from(ip) & mask)
}

fn cidr_contains_v6(network: Ipv6Addr, ip: Ipv6Addr, prefix_len: u8) -> bool {
    let mask = if prefix_len == 0 {
        0
    } else {
        u128::MAX << (128 - prefix_len)
    };
    (u128::from(network) & mask) == (u128::from(ip) & mask)
}

/// In-process sliding-window rate limiter keyed by a stable identity string.
/// Uses DashMap for lock-free concurrent access.
pub struct RateLimiter {
    state: DashMap<String, VecDeque<Instant>>,
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
    pub fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        if !self.state.contains_key(key) && self.state.len() >= MAX_RATE_LIMIT_KEYS {
            self.cleanup();
            if self.state.len() >= MAX_RATE_LIMIT_KEYS {
                return false;
            }
        }
        let mut entry = self.state.entry(key.to_string()).or_default();
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
            timestamps
                .back()
                .is_some_and(|&t| now.duration_since(t) < self.window)
        });
    }
}

/// Extract the client IP from the socket address or from proxy headers only
/// when the direct peer is an explicitly trusted reverse proxy.
fn extract_client_ip(request: &Request, trusted_proxies: &[TrustedProxy]) -> IpAddr {
    let peer_ip = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|connect_info| connect_info.0.ip());

    if peer_ip.is_some_and(|ip| is_trusted_proxy_peer(ip, trusted_proxies))
        && let Some(ip) = header_ip(request, "x-real-ip").or_else(|| forwarded_for_ip(request))
    {
        return ip;
    }

    if let Some(ip) = peer_ip {
        return ip;
    }

    header_ip(request, "x-real-ip")
        .or_else(|| forwarded_for_ip(request))
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
}

fn header_ip(request: &Request, header: &'static str) -> Option<IpAddr> {
    request
        .headers()
        .get(header)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.trim().parse().ok())
}

fn forwarded_for_ip(request: &Request) -> Option<IpAddr> {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
}

fn is_trusted_proxy_peer(ip: IpAddr, trusted_proxies: &[TrustedProxy]) -> bool {
    trusted_proxies.iter().any(|proxy| proxy.contains(ip))
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
        if !self.state.contains_key(key_hash) && self.state.len() >= MAX_RATE_LIMIT_KEYS {
            self.cleanup();
            if self.state.len() >= MAX_RATE_LIMIT_KEYS {
                return false;
            }
        }
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

    pub fn cleanup(&self) {
        let now = Instant::now();
        self.state.retain(|_, timestamps| {
            timestamps
                .back()
                .is_some_and(|&timestamp| now.duration_since(timestamp) < self.window)
        });
    }
}

pub async fn run_cleanup_worker(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        state.api_rate_limiter.cleanup();
        state.login_rate_limiter.cleanup();
        state.ci_rate_limiter.cleanup();
    }
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Skip rate limiting for health probes
    if path == "/health" || path == "/ready" {
        return next.run(request).await;
    }

    let ip = extract_client_ip(&request, &state.trusted_proxies);

    // Stricter limit for login, general limit for everything else
    let (limiter, key) = if path == "/api/v1/auth/login" {
        (&state.login_rate_limiter, format!("ip:{ip}"))
    } else {
        let key = general_rate_limit_key(&mut request, ip, &state.jwt_secret);
        (&state.api_rate_limiter, key)
    };

    if limiter.check(&key) {
        next.run(request).await
    } else {
        crate::error::AppError::TooManyRequests.into_response()
    }
}

fn general_rate_limit_key(request: &mut Request, ip: IpAddr, jwt_secret: &str) -> String {
    if let Some(context) = request.extensions().get::<RequestContext>() {
        return format!("user:{}:{}", context.tenant_id_str(), context.user_id);
    }

    if let Some(mapped_claims) = request.extensions().get::<MappedUserClaims>() {
        let Some(auth_epoch) = mapped_claims
            .claims()
            .auth_epoch
            .as_deref()
            .filter(|value| !value.is_empty())
        else {
            return format!("ip:{ip}");
        };
        return format!(
            "user:{}:{}:{auth_epoch}",
            mapped_claims.tenant_id(),
            mapped_claims.claims().sub
        );
    }

    let Some(mapped_claims) = request_token(request)
        .and_then(|token| crate::auth::validate_token(&token, jwt_secret).ok())
        .and_then(|claims| map_validated_user_claims(claims).ok())
    else {
        return format!("ip:{ip}");
    };
    let Some(auth_epoch) = mapped_claims
        .claims()
        .auth_epoch
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return format!("ip:{ip}");
    };

    let key = format!(
        "user:{}:{}:{auth_epoch}",
        mapped_claims.tenant_id(),
        mapped_claims.claims().sub
    );
    request.extensions_mut().insert(mapped_claims);
    key
}

fn request_token(request: &Request) -> Option<String> {
    request
        .headers()
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .map(str::to_string)
        .or_else(|| {
            request
                .headers()
                .get("cookie")
                .and_then(|header| header.to_str().ok())
                .and_then(|cookies| {
                    cookies.split(';').find_map(|cookie| {
                        let (name, value) = cookie.trim().split_once('=')?;
                        (name == "extrittio_session").then(|| value.to_string())
                    })
                })
        })
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use axum::body::Body;
    use axum::extract::Request;

    use crate::auth::Claims;
    use crate::auth::context::{MappedUserClaims, RequestContext};

    use super::{TrustedProxy, cidr_contains_v4, general_rate_limit_key};

    const JWT_SECRET: &str = "test-rate-limit-secret-with-enough-entropy";

    fn request_with_token(token: &str) -> Request {
        Request::builder()
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }

    fn token(tenant_id: &str) -> String {
        crate::auth::create_token_with_scopes(
            23,
            "operator",
            "viewer",
            tenant_id,
            vec!["devices:read".to_string()],
            1,
            "test-auth-epoch",
            JWT_SECRET,
        )
        .unwrap()
    }

    #[test]
    fn cidr_matching_handles_ipv4_ranges() {
        assert!(cidr_contains_v4(
            Ipv4Addr::new(172, 30, 0, 0),
            Ipv4Addr::new(172, 30, 0, 3),
            24
        ));
        assert!(!cidr_contains_v4(
            Ipv4Addr::new(172, 30, 0, 0),
            Ipv4Addr::new(172, 31, 0, 3),
            24
        ));
    }

    #[test]
    fn trusted_proxy_parses_single_ip_and_cidr() {
        let single = TrustedProxy::parse("172.30.0.3").expect("valid ip");
        assert!(single.contains(IpAddr::V4(Ipv4Addr::new(172, 30, 0, 3))));
        assert!(!single.contains(IpAddr::V4(Ipv4Addr::new(172, 30, 0, 4))));

        let cidr = TrustedProxy::parse("172.30.0.0/24").expect("valid cidr");
        assert!(cidr.contains(IpAddr::V4(Ipv4Addr::new(172, 30, 0, 200))));
        assert!(!cidr.contains(IpAddr::V4(Ipv4Addr::new(172, 30, 1, 1))));
    }

    #[test]
    fn mapped_request_context_is_the_preferred_rate_limit_identity() {
        let mut request = Request::builder().body(Body::empty()).unwrap();
        let context = RequestContext::from_claims(Claims {
            sub: 23,
            username: "operator".to_string(),
            role: "viewer".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            scopes: vec!["devices:read".to_string()],
            permission_version: 1,
            auth_epoch: Some("test-auth-epoch".to_string()),
            exp: usize::MAX,
        })
        .unwrap();
        request.extensions_mut().insert(context);

        let key = general_rate_limit_key(
            &mut request,
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 4)),
            JWT_SECRET,
        );

        assert_eq!(key, "user:tenant-a:23:test-auth-epoch");
    }

    #[test]
    fn signed_token_uses_the_single_mapped_identity_and_caches_it_for_auth() {
        let mut request = request_with_token(&token("tenant-a"));

        let key = general_rate_limit_key(
            &mut request,
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 5)),
            JWT_SECRET,
        );

        assert_eq!(key, "user:tenant-a:23");
        assert!(request.extensions().get::<MappedUserClaims>().is_some());
    }

    #[test]
    fn present_invalid_tenant_claim_uses_ip_instead_of_default_tenant() {
        let mut request = request_with_token(&token(" \t"));

        let key = general_rate_limit_key(
            &mut request,
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 6)),
            JWT_SECRET,
        );

        assert_eq!(key, "ip:203.0.113.6");
        assert!(request.extensions().get::<MappedUserClaims>().is_none());
    }

    #[test]
    fn request_without_a_valid_mapped_identity_uses_ip() {
        let mut request = Request::builder().body(Body::empty()).unwrap();

        let key = general_rate_limit_key(
            &mut request,
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)),
            JWT_SECRET,
        );

        assert_eq!(key, "ip:203.0.113.7");
    }
}
