use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::error::AppError;

pub fn validate_public_https_url(
    raw_url: &str,
    field_name: &str,
) -> Result<reqwest::Url, AppError> {
    let url = reqwest::Url::parse(raw_url)
        .map_err(|_| AppError::BadRequest(format!("{field_name} must be a valid HTTPS URL")))?;

    if url.scheme() != "https" {
        return Err(AppError::BadRequest(format!(
            "{field_name} must use https://"
        )));
    }

    let host = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest(format!("{field_name} must include a host")))?;
    let literal_host = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = literal_host.parse::<IpAddr>()
        && is_blocked_ip(ip)
    {
        return Err(AppError::BadRequest(format!(
            "{field_name} must not target private, loopback, or link-local addresses"
        )));
    }

    Ok(url)
}

pub async fn validate_resolved_public_target(
    url: &reqwest::Url,
) -> Result<Vec<SocketAddr>, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "webhook URL must include a host".to_string())?;
    let port = url.port_or_known_default().unwrap_or(443);

    let resolved = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("failed to resolve webhook host {host}: {e}"))?;

    validate_resolved_addrs(host, resolved)
}

pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match normalize_ip(ip) {
        IpAddr::V4(ip) => is_blocked_ipv4(ip),
        IpAddr::V6(ip) => is_blocked_ipv6(ip),
    }
}

fn validate_resolved_addrs(
    host: &str,
    resolved: impl IntoIterator<Item = SocketAddr>,
) -> Result<Vec<SocketAddr>, String> {
    let mut addrs = Vec::new();
    for addr in resolved {
        let addr = normalize_socket_addr(addr);
        if is_blocked_ip(addr.ip()) {
            return Err(format!(
                "webhook host {host} resolves to a private, loopback, or link-local address"
            ));
        }
        if !addrs.contains(&addr) {
            addrs.push(addr);
        }
    }

    if addrs.is_empty() {
        return Err(format!(
            "webhook host {host} did not resolve to any address"
        ));
    }

    Ok(addrs)
}

fn normalize_socket_addr(addr: SocketAddr) -> SocketAddr {
    SocketAddr::new(normalize_ip(addr.ip()), addr.port())
}

fn normalize_ip(ip: IpAddr) -> IpAddr {
    let IpAddr::V6(ipv6) = ip else {
        return ip;
    };
    let segments = ipv6.segments();
    if segments[..5] == [0; 5] && matches!(segments[5], 0 | 0xffff) {
        let high = segments[6].to_be_bytes();
        let low = segments[7].to_be_bytes();
        IpAddr::V4(Ipv4Addr::new(high[0], high[1], low[0], low[1]))
    } else {
        IpAddr::V6(ipv6)
    }
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || octets[0] == 0
        || octets[0] == 10
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 169 && octets[1] == 254)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        || octets[0] >= 224
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "network policy tests stay adjacent to the address classifiers they cover"
)]
mod tests {
    use super::*;

    #[test]
    fn blocks_mapped_and_compatible_ipv4_special_ranges() {
        for address in [
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::ffff:169.254.1.1",
            "::ffff:0.0.0.0",
            "::ffff:224.0.0.1",
            "::ffff:100.64.0.1",
            "::ffff:240.0.0.1",
            "::127.0.0.1",
            "::10.0.0.1",
        ] {
            assert!(
                is_blocked_ip(address.parse().unwrap()),
                "{address} should be blocked"
            );
        }
        assert!(!is_blocked_ip("::ffff:8.8.8.8".parse().unwrap()));
        assert!(!is_blocked_ip("::8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn literal_mapped_ipv6_urls_are_rejected() {
        let error = validate_public_https_url("https://[::ffff:127.0.0.1]/hook", "webhook url")
            .unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
    }

    #[test]
    fn resolved_mapped_addresses_are_normalized_before_pinning() {
        let public_mapped = SocketAddr::new("::ffff:8.8.8.8".parse().unwrap(), 443);
        let public_v4 = SocketAddr::new("8.8.8.8".parse().unwrap(), 443);
        assert_eq!(
            validate_resolved_addrs("webhook.example", [public_mapped, public_v4]).unwrap(),
            vec![public_v4]
        );

        let private_mapped = SocketAddr::new("::ffff:127.0.0.1".parse().unwrap(), 443);
        assert!(validate_resolved_addrs("webhook.example", [private_mapped]).is_err());
    }

    #[test]
    fn resolved_targets_must_not_be_empty() {
        assert!(validate_resolved_addrs("webhook.example", []).is_err());
    }
}

/// Rule configuration validation uses the same URL policy as webhook dispatch.
pub struct PublicWebhookUrlPolicy;
impl extrittio_backend_core::rules::WebhookUrlPolicy for PublicWebhookUrlPolicy {
    fn validate(&self, url: &str) -> Result<(), extrittio_backend_core::ApplicationError> {
        validate_public_https_url(url, "webhook url")
            .map(|_| ())
            .map_err(|error| match error {
                AppError::BadRequest(message) => {
                    extrittio_backend_core::ApplicationError::InvalidInput(message)
                }
                other => extrittio_backend_core::ApplicationError::Internal(other.to_string()),
            })
    }
}
