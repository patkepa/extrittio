/// Valid device status values for heartbeat messages.
pub const ONLINE: &str = "online";
pub const OFFLINE: &str = "offline";
pub const WARNING: &str = "warning";

pub const VALID_STATUSES: &[&str] = &[ONLINE, OFFLINE, WARNING];

/// Returns `true` if the given status string is a known valid status.
pub fn is_valid(status: &str) -> bool {
    VALID_STATUSES.iter().any(|&s| s.eq_ignore_ascii_case(status))
}
