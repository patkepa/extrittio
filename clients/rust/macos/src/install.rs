use std::path::PathBuf;

use crate::config::Config;

const PLIST_LABEL: &str = "io.extrittio.agent";

fn plist_path() -> PathBuf {
    dirs::home_dir()
        .expect("Cannot determine home directory")
        .join("Library/LaunchAgents/io.extrittio.agent.plist")
}

fn log_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Cannot determine home directory")
        .join("Library/Logs/extrittio")
}

/// Generate the LaunchAgent plist XML content.
fn generate_plist(exe_path: &str, home: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PLIST_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_path}</string>
        <string>run</string>
    </array>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{home}/Library/Logs/extrittio/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{home}/Library/Logs/extrittio/stderr.log</string>
</dict>
</plist>
"#
    )
}

/// Install the LaunchAgent: create config dir, write default config, write plist, load agent.
pub fn install(device_id: Option<&str>, connect: Option<&str>, contract_path: &str) {
    let contract_path = std::fs::canonicalize(contract_path).expect("contract file must exist");
    let contract = extrittio_client_runtime::contract::ProvisionedContract::from_api_response(
        &std::fs::read(&contract_path).expect("read provisioned contract"),
    )
    .expect("valid provisioned contract required");
    if let Some(id) = device_id {
        contract
            .validate_device_id(id)
            .expect("device identity must match contract");
    }
    let config_dir = Config::config_dir();
    let config_path = Config::config_path();
    let log_directory = log_dir();
    let plist = plist_path();

    // 1. Config directory + default config
    std::fs::create_dir_all(&config_dir)
        .unwrap_or_else(|e| panic!("Failed to create {}: {e}", config_dir.display()));

    if !config_path.exists() {
        let mut cfg = Config::default();
        cfg.contract_path = contract_path.to_string_lossy().into_owned();
        if let Some(id) = device_id {
            cfg.device_id = Some(id.to_string());
        }
        if let Some(ep) = connect {
            cfg.connect = Some(ep.to_string());
        }
        cfg.save(&config_path)
            .unwrap_or_else(|e| panic!("Failed to write config: {e}"));
        println!("Config written to {}", config_path.display());
    } else {
        let mut cfg = Config::load();
        cfg.contract_path = contract_path.to_string_lossy().into_owned();
        cfg.device_id = Some(contract.device_id().to_string());
        if let Some(endpoint) = connect {
            cfg.connect = Some(endpoint.to_string());
        }
        cfg.save(&config_path).expect("save contract configuration");
    }

    // 2. Log directory
    std::fs::create_dir_all(&log_directory)
        .unwrap_or_else(|e| panic!("Failed to create {}: {e}", log_directory.display()));

    // 3. Plist
    let exe_path = std::env::current_exe()
        .expect("Cannot resolve current executable path")
        .to_string_lossy()
        .to_string();
    let home = dirs::home_dir()
        .expect("Cannot determine home directory")
        .to_string_lossy()
        .to_string();

    let plist_content = generate_plist(&exe_path, &home);

    // Ensure LaunchAgents directory exists
    if let Some(parent) = plist.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("Failed to create {}: {e}", parent.display()));
    }

    std::fs::write(&plist, &plist_content).unwrap_or_else(|e| panic!("Failed to write plist: {e}"));
    println!("Plist written to {}", plist.display());

    // 4. Load the agent
    let uid = unsafe { libc::getuid() };
    let status = std::process::Command::new("launchctl")
        .args(["bootstrap", &format!("gui/{uid}"), &plist.to_string_lossy()])
        .status();

    match status {
        Ok(s) if s.success() => println!("LaunchAgent loaded successfully."),
        Ok(s) => {
            // Exit code 37 means "already loaded" — that's fine
            if s.code() == Some(37) {
                println!("LaunchAgent was already loaded.");
            } else {
                eprintln!("launchctl bootstrap exited with: {s}");
            }
        }
        Err(e) => eprintln!("Failed to run launchctl: {e}"),
    }
}

/// Uninstall the LaunchAgent: unload + remove plist. Keeps config.
pub fn uninstall() {
    let plist = plist_path();
    let uid = unsafe { libc::getuid() };

    let status = std::process::Command::new("launchctl")
        .args(["bootout", &format!("gui/{uid}/{PLIST_LABEL}")])
        .status();

    match status {
        Ok(s) if s.success() => println!("LaunchAgent unloaded."),
        Ok(s) => eprintln!("launchctl bootout exited with: {s}"),
        Err(e) => eprintln!("Failed to run launchctl: {e}"),
    }

    if plist.exists() {
        std::fs::remove_file(&plist).unwrap_or_else(|e| eprintln!("Failed to remove plist: {e}"));
        println!("Plist removed.");
    }

    println!("Config preserved at {}", Config::config_dir().display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_contains_label_and_paths() {
        let content = generate_plist("/usr/local/bin/extrittio-macos", "/Users/test");
        assert!(content.contains("io.extrittio.agent"));
        assert!(content.contains("/usr/local/bin/extrittio-macos"));
        assert!(content.contains("/Users/test/Library/Logs/extrittio/stdout.log"));
        assert!(content.contains("<string>run</string>"));
        assert!(content.contains("<key>KeepAlive</key>"));
        assert!(content.contains("<key>RunAtLoad</key>"));
    }
}
