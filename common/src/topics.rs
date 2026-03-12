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
