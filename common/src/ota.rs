/// OTA status values used in shadow reports.
pub mod status {
    pub const DOWNLOADING: &str = "downloading";
    pub const VERIFYING: &str = "verifying";
    pub const INSTALLING: &str = "installing";
    pub const SUCCESS: &str = "success";
    pub const FAILED: &str = "failed";

    /// Returns true if the status is terminal (success or failed).
    pub fn is_terminal(status: &str) -> bool {
        status.eq_ignore_ascii_case(SUCCESS) || status.eq_ignore_ascii_case(FAILED)
    }
}

/// Field names used in OTA shadow payloads.
pub mod fields {
    /// The shadow key under which OTA payloads are stored.
    pub const SHADOW_KEY: &str = "ota";
    pub const FIRMWARE_VERSION: &str = "firmware_version";
    pub const FIRMWARE_URL: &str = "firmware_url";
    pub const FIRMWARE_UPDATE_ID: &str = "firmware_update_id";
    pub const SHA256: &str = "sha256";
    pub const STATUS: &str = "status";
    pub const ERROR: &str = "error";
}

#[cfg(test)]
mod tests {
    use super::status::*;

    #[test]
    fn test_is_terminal_exact() {
        assert!(is_terminal(SUCCESS));
        assert!(is_terminal(FAILED));
        assert!(!is_terminal(DOWNLOADING));
        assert!(!is_terminal(VERIFYING));
        assert!(!is_terminal(INSTALLING));
    }

    #[test]
    fn test_is_terminal_case_insensitive() {
        assert!(is_terminal("SUCCESS"));
        assert!(is_terminal("Failed"));
        assert!(is_terminal("FAILED"));
        assert!(is_terminal("Success"));
        assert!(!is_terminal("DOWNLOADING"));
        assert!(!is_terminal("Verifying"));
    }
}
