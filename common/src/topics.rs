#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::format;
#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::string::String;

/// Build a device-specific telemetry topic.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn telemetry(device_id: &str) -> String {
    format!("extrittio/devices/{device_id}/telemetry")
}

/// Build a device-specific heartbeat topic.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn heartbeat(device_id: &str) -> String {
    format!("extrittio/devices/{device_id}/heartbeat")
}

/// Build a device-specific shadow get topic.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn shadow_get(device_id: &str) -> String {
    format!("extrittio/devices/{device_id}/shadow/get")
}

/// Build a device-specific shadow delta topic.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn shadow_delta(device_id: &str) -> String {
    format!("extrittio/devices/{device_id}/shadow/delta")
}

/// Build a device-specific shadow report topic.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn shadow_report(device_id: &str) -> String {
    format!("extrittio/devices/{device_id}/shadow/report")
}

/// Build a device-specific logs topic.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn logs(device_id: &str) -> String {
    format!("extrittio/devices/{device_id}/logs")
}

/// Build a device-specific commands topic.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn commands(device_id: &str) -> String {
    format!("extrittio/devices/{device_id}/commands")
}

/// Build a device-specific command response topic.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn commands_response(device_id: &str) -> String {
    format!("extrittio/devices/{device_id}/commands/response")
}

const DEVICE_TOPIC_PREFIX: &str = "extrittio/devices/";

/// Validate a device ID for use in Zenoh topics and REST path segments.
///
/// Device IDs are intentionally restricted to a compact ASCII set so they
/// cannot change topic hierarchy or introduce wildcard semantics.
#[must_use]
pub fn is_valid_device_id(device_id: &str) -> bool {
    let len = device_id.len();
    (1..=128).contains(&len)
        && device_id.bytes().all(|byte| {
            matches!(
                byte,
                b'a'..=b'z'
                    | b'A'..=b'Z'
                    | b'0'..=b'9'
                    | b'-'
                    | b'_'
                    | b'.'
                    | b':'
            )
        })
}

fn device_id_for_suffix<'a>(topic: &'a str, suffix: &str) -> Option<&'a str> {
    let rest = topic.strip_prefix(DEVICE_TOPIC_PREFIX)?;
    let device_id = rest.strip_suffix(suffix)?;
    is_valid_device_id(device_id).then_some(device_id)
}

#[must_use]
pub fn telemetry_device_id(topic: &str) -> Option<&str> {
    device_id_for_suffix(topic, "/telemetry")
}

#[must_use]
pub fn heartbeat_device_id(topic: &str) -> Option<&str> {
    device_id_for_suffix(topic, "/heartbeat")
}

#[must_use]
pub fn shadow_report_device_id(topic: &str) -> Option<&str> {
    device_id_for_suffix(topic, "/shadow/report")
}

#[must_use]
pub fn shadow_get_device_id(topic: &str) -> Option<&str> {
    device_id_for_suffix(topic, "/shadow/get")
}

#[must_use]
pub fn logs_device_id(topic: &str) -> Option<&str> {
    device_id_for_suffix(topic, "/logs")
}

#[must_use]
pub fn commands_response_device_id(topic: &str) -> Option<&str> {
    device_id_for_suffix(topic, "/commands/response")
}

/// Wildcard topic patterns for backend subscriptions.
pub mod patterns {
    pub const TELEMETRY: &str = "extrittio/devices/*/telemetry";
    pub const HEARTBEAT: &str = "extrittio/devices/*/heartbeat";
    pub const SHADOW_REPORT: &str = "extrittio/devices/*/shadow/report";
    pub const SHADOW_GET: &str = "extrittio/devices/*/shadow/get";
    pub const SHADOW_DELTA: &str = "extrittio/devices/*/shadow/delta";
    pub const LOGS: &str = "extrittio/devices/*/logs";
    pub const COMMANDS_RESPONSE: &str = "extrittio/devices/*/commands/response";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_topic_safe_device_ids() {
        assert!(is_valid_device_id("dev-001"));
        assert!(is_valid_device_id("mac.device:01"));
        assert!(!is_valid_device_id(""));
        assert!(!is_valid_device_id("dev/001"));
        assert!(!is_valid_device_id("dev*"));
        assert!(!is_valid_device_id("dev 001"));
    }

    #[test]
    fn extracts_device_id_from_expected_topics() {
        assert_eq!(
            telemetry_device_id("extrittio/devices/dev-001/telemetry"),
            Some("dev-001")
        );
        assert_eq!(
            commands_response_device_id("extrittio/devices/dev-001/commands/response"),
            Some("dev-001")
        );
        assert_eq!(
            shadow_report_device_id("extrittio/devices/dev/001/shadow/report"),
            None
        );
        assert_eq!(logs_device_id("extrittio/devices/dev-001/telemetry"), None);
    }
}
