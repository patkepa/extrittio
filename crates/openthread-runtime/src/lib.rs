//! Lifecycle support for a local OpenThread Border Router (OTBR) radio co-processor.
//!
//! The Thread network itself is implemented by OpenThread's `otbr-agent`. This crate
//! deliberately owns only Extrittio-specific concerns: deterministic RCP discovery,
//! safe construction of the radio URL, and child-process lifecycle management. A
//! raw Spinel client is insufficient for a border router because OTBR also configures
//! IPv6 routing, service discovery, and the host network interface.

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use anyhow::{Context, Result, bail};
use tracing::{info, warn};

/// Configuration for one local OTBR instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderRouterConfig {
    /// OpenThread's `otbr-agent` executable.
    pub agent_path: PathBuf,
    /// Serial RCP device, such as `/dev/cu.usbmodem…` or `/dev/ttyACM0`.
    pub rcp_device: PathBuf,
    /// UART speed configured in the RCP firmware.
    pub baud_rate: u32,
    /// Name of the host-side Thread interface created by OTBR.
    pub thread_interface: String,
    /// Ethernet or Wi-Fi interface adjacent to the Extrittio host.
    pub infrastructure_interface: String,
}

impl BorderRouterConfig {
    #[must_use]
    pub fn radio_url(&self) -> String {
        format!(
            "spinel+hdlc+uart://{}?uart-baudrate={}",
            self.rcp_device.display(),
            self.baud_rate
        )
    }

    /// The argument sequence accepted by OpenThread's portable `otbr-agent`.
    #[must_use]
    pub fn agent_args(&self) -> Vec<OsString> {
        vec![
            "-I".into(),
            self.thread_interface.clone().into(),
            "-B".into(),
            self.infrastructure_interface.clone().into(),
            self.radio_url().into(),
        ]
    }
}

/// A running `otbr-agent`, stopped together with the Extrittio hobby process.
pub struct BorderRouter {
    child: Child,
    config: BorderRouterConfig,
}

impl BorderRouter {
    /// Start OTBR and confirm that it did not fail immediately.
    pub fn start(config: BorderRouterConfig) -> Result<Self> {
        validate_config(&config)?;
        let args = config.agent_args();
        info!(
            rcp = %config.rcp_device.display(),
            interface = %config.thread_interface,
            infrastructure_interface = %config.infrastructure_interface,
            "Starting OpenThread border router"
        );
        let mut child = Command::new(&config.agent_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to start OpenThread border router at {}",
                    config.agent_path.display()
                )
            })?;

        if let Some(status) = child
            .try_wait()
            .context("Failed to inspect OpenThread border router startup")?
        {
            bail!("OpenThread border router exited immediately with status {status}");
        }

        Ok(Self { child, config })
    }

    #[must_use]
    pub fn config(&self) -> &BorderRouterConfig {
        &self.config
    }

    /// Stop OTBR when Extrittio exits. OTBR is a child of the hobby process and
    /// must not be left behind with routes pointing to a disconnected RCP.
    pub fn shutdown(&mut self) -> Result<()> {
        if self
            .child
            .try_wait()
            .context("Failed to inspect OpenThread border router")?
            .is_none()
        {
            self.child
                .kill()
                .context("Failed to stop OpenThread border router")?;
            self.child
                .wait()
                .context("Failed to wait for OpenThread border router shutdown")?;
        }
        Ok(())
    }
}

impl Drop for BorderRouter {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            warn!(%error, "Failed to stop OpenThread border router during shutdown");
        }
    }
}

/// Finds a directly connected serial RCP when there is exactly one plausible
/// candidate. Returns `None` instead of guessing when several devices exist.
pub fn discover_rcp() -> Result<Option<PathBuf>> {
    let candidates = serial_candidates()?;
    match candidates.as_slice() {
        [] => Ok(None),
        [candidate] => Ok(Some(candidate.clone())),
        _ => {
            warn!(
                candidates = ?candidates,
                "Multiple serial devices could be Thread RCPs; set --thread-rcp explicitly"
            );
            Ok(None)
        }
    }
}

/// Resolves the OTBR executable. Packaged hobby releases place it beside the
/// application; a user override and a PATH-installed copy remain useful during
/// local development.
pub fn discover_agent(explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        return executable_if_present(path).map(Some);
    }
    if let Some(path) = env::var_os("EXTRITTIO_OTBR_AGENT") {
        return executable_if_present(Path::new(&path)).map(Some);
    }

    for path in bundled_agent_paths() {
        if path.is_file() {
            return Ok(Some(path));
        }
    }
    Ok(find_in_path("otbr-agent"))
}

/// Selects the usual primary network interface for a standalone hobby host.
#[must_use]
pub fn default_infrastructure_interface() -> &'static str {
    if cfg!(target_os = "macos") {
        "en0"
    } else {
        "eth0"
    }
}

fn validate_config(config: &BorderRouterConfig) -> Result<()> {
    if config.baud_rate == 0 {
        bail!("OpenThread RCP baud rate must be greater than zero");
    }
    if config.thread_interface.trim().is_empty()
        || config.infrastructure_interface.trim().is_empty()
    {
        bail!("OpenThread interface names must not be empty");
    }
    executable_if_present(&config.agent_path)?;
    if !config.rcp_device.exists() {
        bail!(
            "OpenThread RCP device does not exist: {}",
            config.rcp_device.display()
        );
    }
    Ok(())
}

fn executable_if_present(path: &Path) -> Result<PathBuf> {
    if !path.is_file() {
        bail!(
            "OpenThread border-router executable is missing: {}",
            path.display()
        );
    }
    Ok(path.to_path_buf())
}

fn bundled_agent_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = env::current_exe() {
        if let Some(bin_dir) = executable.parent() {
            paths.push(bin_dir.join("../libexec/extrittio/otbr-agent"));
            paths.push(bin_dir.join("otbr-agent"));
        }
    }
    paths
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(program))
            .find(|candidate| candidate.is_file())
    })
}

fn serial_candidates() -> Result<Vec<PathBuf>> {
    #[cfg(target_os = "macos")]
    let roots = ["/dev"];
    #[cfg(target_os = "linux")]
    let roots = ["/dev/serial/by-id", "/dev"];
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let roots: [&str; 0] = [];

    let mut candidates = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries {
            let entry = entry.context("Failed to inspect serial device")?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if is_rcp_candidate_name(&name) {
                candidates.push(entry.path());
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn is_rcp_candidate_name(name: &str) -> bool {
    #[cfg(target_os = "macos")]
    return name.starts_with("cu.usbmodem") || name.starts_with("cu.usbserial");
    #[cfg(target_os = "linux")]
    return name.contains("Nordic")
        || name.contains("nRF")
        || name.starts_with("ttyACM")
        || name.starts_with("ttyUSB");
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    false
}

#[cfg(test)]
mod tests {
    use super::{BorderRouterConfig, default_infrastructure_interface};
    use std::path::PathBuf;

    #[test]
    fn builds_the_standard_spinel_uart_url_and_otbr_arguments() {
        let config = BorderRouterConfig {
            agent_path: "/opt/extrittio/libexec/otbr-agent".into(),
            rcp_device: PathBuf::from("/dev/cu.usbmodem14101"),
            baud_rate: 460_800,
            thread_interface: "wpan0".into(),
            infrastructure_interface: "en0".into(),
        };

        assert_eq!(
            config.radio_url(),
            "spinel+hdlc+uart:///dev/cu.usbmodem14101?uart-baudrate=460800"
        );
        assert_eq!(
            config.agent_args(),
            vec![
                "-I",
                "wpan0",
                "-B",
                "en0",
                "spinel+hdlc+uart:///dev/cu.usbmodem14101?uart-baudrate=460800"
            ]
        );
    }

    #[test]
    fn chooses_a_sensible_default_infrastructure_interface() {
        assert!(matches!(default_infrastructure_interface(), "en0" | "eth0"));
    }
}
