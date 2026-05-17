use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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
    if let Ok(ip) = host.parse::<IpAddr>()
        && is_blocked_ip(ip)
    {
        return Err(AppError::BadRequest(format!(
            "{field_name} must not target private, loopback, or link-local addresses"
        )));
    }

    Ok(url)
}

pub async fn validate_resolved_public_target(url: &reqwest::Url) -> Result<(), String> {
    let host = url
        .host_str()
        .ok_or_else(|| "webhook URL must include a host".to_string())?;
    let port = url.port_or_known_default().unwrap_or(443);

    let resolved = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("failed to resolve webhook host {host}: {e}"))?;

    let mut saw_addr = false;
    for addr in resolved {
        saw_addr = true;
        if is_blocked_ip(addr.ip()) {
            return Err(format!(
                "webhook host {host} resolves to a private, loopback, or link-local address"
            ));
        }
    }

    if !saw_addr {
        return Err(format!(
            "webhook host {host} did not resolve to any address"
        ));
    }

    Ok(())
}

pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_blocked_ipv4(ip),
        IpAddr::V6(ip) => is_blocked_ipv6(ip),
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
