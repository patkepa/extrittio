use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub contract_path: String,
    pub device_id: Option<String>,
    pub connect: Option<String>,
    #[serde(default = "default_telemetry_interval")]
    pub telemetry_interval_secs: u64,
    #[serde(default = "default_firmware_version")]
    pub firmware_version: String,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TlsConfig {
    pub ca_cert: Option<String>,
    pub client_cert: Option<String>,
    pub client_key: Option<String>,
}

fn default_telemetry_interval() -> u64 {
    5
}

fn default_firmware_version() -> String {
    "v1.0.0-macos".to_string()
}

impl Config {
    /// Returns the platform config directory: ~/Library/Application Support/extrittio/
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .expect("Cannot determine home directory")
            .join("Library/Application Support/extrittio")
    }

    /// Returns the config file path.
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load config from the default path. Returns default config if file doesn't exist.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read config at {}: {e}", path.display()));
            toml::from_str(&contents)
                .unwrap_or_else(|e| panic!("Failed to parse config at {}: {e}", path.display()))
        } else {
            Self::default()
        }
    }

    /// Write this config to a file.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let contents = toml::to_string_pretty(self).expect("Failed to serialize config");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_id: None,
            connect: None,
            telemetry_interval_secs: default_telemetry_interval(),
            contract_path: String::new(),
            firmware_version: default_firmware_version(),
            tls: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_contract_and_retired_heartbeat_override() {
        assert!(toml::from_str::<Config>("device_id = 'mac-1'").is_err());
        assert!(
            toml::from_str::<Config>(
                "contract_path = '/contract.json'\nheartbeat_interval_secs = 30"
            )
            .is_err()
        );
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
device_id = "mac-001"
connect = "tcp/192.0.2.10:7447"
telemetry_interval_secs = 10
contract_path = "/path/to/contract.json"
firmware_version = "v2.0.0-macos"

[tls]
ca_cert = "/path/to/ca.pem"
client_cert = "/path/to/device.pem"
client_key = "/path/to/device.key"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.device_id.as_deref(), Some("mac-001"));
        assert_eq!(cfg.connect.as_deref(), Some("tcp/192.0.2.10:7447"));
        assert_eq!(cfg.telemetry_interval_secs, 10);
        assert_eq!(cfg.contract_path, "/path/to/contract.json");
        assert_eq!(cfg.firmware_version, "v2.0.0-macos");
        let tls = cfg.tls.unwrap();
        assert_eq!(tls.ca_cert.as_deref(), Some("/path/to/ca.pem"));
        assert_eq!(tls.client_cert.as_deref(), Some("/path/to/device.pem"));
        assert_eq!(tls.client_key.as_deref(), Some("/path/to/device.key"));
    }

    #[test]
    fn parse_minimal_config() {
        let toml = r#"
device_id = "mac-002"
contract_path = "/path/to/contract.json"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.device_id.as_deref(), Some("mac-002"));
        assert_eq!(cfg.connect, None);
        assert_eq!(cfg.telemetry_interval_secs, 5);
        assert_eq!(cfg.firmware_version, "v1.0.0-macos");
        assert!(cfg.tls.is_none());
    }

    #[test]
    fn default_config_has_no_device_id() {
        let cfg = Config::default();
        assert!(cfg.device_id.is_none());
        assert_eq!(cfg.telemetry_interval_secs, 5);
    }

    #[test]
    fn roundtrip_serialize() {
        let cfg = Config {
            device_id: Some("test-mac".into()),
            connect: Some("tcp/localhost:7447".into()),
            ..Config::default()
        };
        let serialized = toml::to_string_pretty(&cfg).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.device_id.as_deref(), Some("test-mac"));
        assert_eq!(deserialized.connect.as_deref(), Some("tcp/localhost:7447"));
    }
}
