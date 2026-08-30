use std::{
    ffi::OsString,
    fmt::Write as _,
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::fd::AsFd as _,
    os::unix::{fs::FileTypeExt as _, process::CommandExt as _},
    path::{Path, PathBuf},
    process::{Child, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, poll},
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use reqwest::Url;
use secrecy::{ExposeSecret, SecretString};
use tracing::{info, warn};
use zeroize::Zeroize as _;

use crate::{
    controller::ThreadController,
    dbus::OtbrDbus,
    discovery::executable_if_present,
    error::{OpenThreadError, Result},
    rest::OtbrRest,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
// An ESP32 RCP can need several seconds to recover after a USB reconnect.
// OTBR writes this handoff only after its REST listener is actually bound, so
// do not discard an otherwise healthy router with an unnecessarily short
// startup deadline.
const REST_PORT_TIMEOUT: Duration = Duration::from_secs(15);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
const LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BorderRouterConfig {
    pub agent_path: PathBuf,
    pub rcp_device: PathBuf,
    pub baud_rate: u32,
    pub thread_interface: String,
    pub infrastructure_interface: String,
    pub data_path: PathBuf,
}

impl BorderRouterConfig {
    fn radio_url(&self) -> Result<String> {
        let device = self.rcp_device.to_str().ok_or_else(|| {
            OpenThreadError::InvalidConfiguration(
                "RCP device path must be valid UTF-8 for the OTBR radio URL".to_string(),
            )
        })?;
        if !device.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'.' | b':' | b'+')
        }) {
            return Err(OpenThreadError::InvalidConfiguration(format!(
                "RCP device path contains characters unsupported by the OTBR radio URL: {}",
                self.rcp_device.display()
            )));
        }
        Ok(format!(
            "spinel+hdlc+uart://{}?uart-baudrate={}",
            device, self.baud_rate
        ))
    }

    fn agent_args(&self, rest_port: u16) -> Result<Vec<OsString>> {
        Ok(vec![
            "-I".into(),
            self.thread_interface.clone().into(),
            "-B".into(),
            self.infrastructure_interface.clone().into(),
            "--vendor-name".into(),
            "Extrittio".into(),
            "--model-name".into(),
            "Extrittio Edge".into(),
            "--syslog-disable".into(),
            "--data-path".into(),
            self.data_path.clone().into_os_string(),
            "--rest-listen-address".into(),
            "127.0.0.1".into(),
            "--rest-listen-port".into(),
            rest_port.to_string().into(),
            self.radio_url()?.into(),
        ])
    }
}

pub(crate) struct BorderRouter {
    child: Child,
    config: BorderRouterConfig,
    dbus_daemon: Option<DbusDaemon>,
    log_capture: Option<LogCapture>,
    last_exit: Option<String>,
}

pub(crate) trait ManagedBorderRouter: Send {
    fn rcp_device(&self) -> &Path;
    fn thread_interface(&self) -> &str;
    fn poll_exit(&mut self) -> Result<Option<String>>;
    fn shutdown(&mut self) -> Result<()>;
    fn diagnostic_tail(&self) -> String;
}

impl BorderRouter {
    pub(crate) fn start(config: BorderRouterConfig) -> Result<(Self, ThreadController)> {
        validate_config(&config)?;
        let (dbus_daemon, dbus_address) = DbusDaemon::start()?;
        let rest_port_file = tempfile::Builder::new()
            .prefix("otbr-rest-port-")
            .tempfile_in(&config.data_path)
            .map_err(|source| OpenThreadError::FileSystem {
                operation: "create OpenThread REST port handoff",
                path: config.data_path.clone(),
                source,
            })?;
        let rest_token = generate_rest_token()?;
        let args = config.agent_args(0)?;
        info!(
            rcp = %config.rcp_device.display(),
            interface = %config.thread_interface,
            infrastructure_interface = %config.infrastructure_interface,
            "Starting OpenThread border router"
        );

        let mut command = Command::new(&config.agent_path);
        command
            .args(&args)
            .env("DBUS_SYSTEM_BUS_ADDRESS", &dbus_address)
            .env("EXTRITTIO_OTBR_REST_TOKEN", rest_token.expose_secret())
            .env("EXTRITTIO_OTBR_REST_PORT_FILE", rest_port_file.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.process_group(0);
        let child = command
            .spawn()
            .map_err(|source| OpenThreadError::FileSystem {
                operation: "start OpenThread border router",
                path: config.agent_path.clone(),
                source,
            })?;
        let mut router = Self {
            child,
            config,
            dbus_daemon: Some(dbus_daemon),
            log_capture: None,
            last_exit: None,
        };
        let log_path = router.config.data_path.join("otbr-agent.log");
        let stdout = router.child.stdout.take().ok_or_else(|| {
            OpenThreadError::Process("failed to capture otbr-agent stdout".to_string())
        })?;
        let stderr = router.child.stderr.take().ok_or_else(|| {
            OpenThreadError::Process("failed to capture otbr-agent stderr".to_string())
        })?;
        router.log_capture = Some(LogCapture::start(log_path.clone(), stdout, stderr)?);

        let rest_port = router.wait_for_rest_port(rest_port_file.path(), &log_path)?;
        let rest_endpoint =
            Url::parse(&format!("http://127.0.0.1:{rest_port}/")).map_err(|error| {
                OpenThreadError::InvalidConfiguration(format!(
                    "invalid OTBR REST endpoint: {error}"
                ))
            })?;
        let dbus = OtbrDbus::connect(&dbus_address, &router.config.thread_interface)?;
        let rest = OtbrRest::new(rest_endpoint, rest_token)?;
        let controller = ThreadController::new(dbus, rest, router.config.thread_interface.clone());
        router.wait_until_ready(&controller, &log_path)?;
        Ok((router, controller))
    }

    fn wait_for_rest_port(&mut self, port_file: &Path, log_path: &Path) -> Result<u16> {
        let started = Instant::now();
        loop {
            if let Some(status) =
                self.child
                    .try_wait()
                    .map_err(|source| OpenThreadError::FileSystem {
                        operation: "inspect OpenThread REST startup",
                        path: self.config.agent_path.clone(),
                        source,
                    })?
            {
                return Err(OpenThreadError::Process(format!(
                    "otbr-agent exited with status {status} before reporting its REST port: {}",
                    startup_log_tail(log_path)
                )));
            }
            match fs::read_to_string(port_file) {
                Ok(value) if !value.trim().is_empty() => {
                    return value
                        .trim()
                        .parse::<u16>()
                        .ok()
                        .filter(|port| *port != 0)
                        .ok_or_else(|| {
                            OpenThreadError::InvalidResponse(
                                "otbr-agent reported an invalid loopback REST port".to_string(),
                            )
                        });
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(OpenThreadError::FileSystem {
                        operation: "read OpenThread REST port handoff",
                        path: port_file.to_path_buf(),
                        source,
                    });
                }
            }
            if started.elapsed() >= REST_PORT_TIMEOUT {
                return Err(OpenThreadError::Process(format!(
                    "otbr-agent did not report a bound REST port within {} seconds: {}",
                    REST_PORT_TIMEOUT.as_secs(),
                    startup_log_tail(log_path)
                )));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn wait_until_ready(&mut self, controller: &ThreadController, log_path: &Path) -> Result<()> {
        let started = Instant::now();
        loop {
            if let Some(status) =
                self.child
                    .try_wait()
                    .map_err(|source| OpenThreadError::FileSystem {
                        operation: "inspect OpenThread border router startup",
                        path: self.config.agent_path.clone(),
                        source,
                    })?
            {
                return Err(OpenThreadError::Process(format!(
                    "otbr-agent exited with status {status} during startup: {}",
                    startup_log_tail(log_path)
                )));
            }
            match controller.startup_health_check() {
                Ok(()) => {
                    info!(
                        interface = %self.config.thread_interface,
                        "OpenThread D-Bus and REST control endpoints are ready"
                    );
                    return Ok(());
                }
                Err(error) if started.elapsed() >= STARTUP_TIMEOUT => {
                    return Err(OpenThreadError::Process(format!(
                        "otbr-agent did not become ready within {} seconds: {error}. {}",
                        STARTUP_TIMEOUT.as_secs(),
                        startup_log_tail(log_path)
                    )));
                }
                Err(_) => {}
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn poll_exit_status(&mut self) -> Result<Option<String>> {
        if self.last_exit.is_some() {
            return Ok(self.last_exit.clone());
        }
        let status = self
            .child
            .try_wait()
            .map_err(|source| OpenThreadError::FileSystem {
                operation: "inspect OpenThread border router",
                path: self.config.agent_path.clone(),
                source,
            })?;
        if let Some(status) = status {
            self.last_exit = Some(status.to_string());
        }
        Ok(self.last_exit.clone())
    }

    pub(crate) fn shutdown(&mut self) -> Result<()> {
        let agent_result = terminate_child(&mut self.child, "otbr-agent");
        if agent_result.is_ok()
            && let Some(log_capture) = self.log_capture.take()
        {
            log_capture.finish();
        }
        let daemon_result = self
            .dbus_daemon
            .take()
            .map_or(Ok(()), |mut daemon| daemon.shutdown());
        agent_result.and(daemon_result)
    }
}

impl ManagedBorderRouter for BorderRouter {
    fn rcp_device(&self) -> &Path {
        &self.config.rcp_device
    }

    fn thread_interface(&self) -> &str {
        &self.config.thread_interface
    }

    fn poll_exit(&mut self) -> Result<Option<String>> {
        self.poll_exit_status()
    }

    fn shutdown(&mut self) -> Result<()> {
        Self::shutdown(self)
    }

    fn diagnostic_tail(&self) -> String {
        startup_log_tail(&self.config.data_path.join("otbr-agent.log"))
    }
}

impl Drop for BorderRouter {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            warn!(%error, "Failed to stop OpenThread border router during shutdown");
        }
    }
}

struct DbusDaemon {
    child: Child,
    _stdout: ChildStdout,
    _socket_dir: tempfile::TempDir,
}

impl DbusDaemon {
    fn start() -> Result<(Self, String)> {
        let socket_dir = tempfile::Builder::new()
            .prefix("extrittio-otbr-bus-")
            .tempdir()
            .map_err(|source| OpenThreadError::FileSystem {
                operation: "create private OpenThread D-Bus directory",
                path: std::env::temp_dir(),
                source,
            })?;
        let socket_path = socket_dir.path().to_str().ok_or_else(|| {
            OpenThreadError::InvalidConfiguration(
                "private OpenThread D-Bus directory must be valid UTF-8".to_string(),
            )
        })?;
        let address_arg = format!("--address=unix:dir={socket_path}");
        let mut child = Command::new("dbus-daemon")
            .args([
                "--session",
                address_arg.as_str(),
                "--nofork",
                "--print-address=1",
                "--nopidfile",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|source| OpenThreadError::FileSystem {
                operation: "start private OpenThread D-Bus daemon",
                path: PathBuf::from("dbus-daemon"),
                source,
            })?;
        let Some(stdout) = child.stdout.take() else {
            let _ = terminate_child(&mut child, "dbus-daemon");
            return Err(OpenThreadError::Process(
                "D-Bus daemon did not expose its address pipe".to_string(),
            ));
        };
        let mut reader = BufReader::new(stdout);
        let ready_result = {
            let mut descriptors = [PollFd::new(
                reader.get_ref().as_fd(),
                PollFlags::POLLIN | PollFlags::POLLHUP,
            )];
            poll(&mut descriptors, 2_000_u16)
        };
        let ready = match ready_result {
            Ok(ready) => ready,
            Err(error) => {
                let _ = terminate_child(&mut child, "dbus-daemon");
                return Err(OpenThreadError::Process(format!(
                    "failed while waiting for the private D-Bus address: {error}"
                )));
            }
        };
        if ready == 0 {
            let _ = terminate_child(&mut child, "dbus-daemon");
            return Err(OpenThreadError::Process(
                "D-Bus daemon did not provide its address within two seconds".to_string(),
            ));
        }
        let mut address = String::new();
        if let Err(source) = reader.read_line(&mut address) {
            let _ = terminate_child(&mut child, "dbus-daemon");
            return Err(OpenThreadError::FileSystem {
                operation: "read private OpenThread D-Bus address",
                path: PathBuf::from("dbus-daemon"),
                source,
            });
        }
        let address = address.trim().to_string();
        if address.is_empty() {
            let _ = terminate_child(&mut child, "dbus-daemon");
            return Err(OpenThreadError::Process(
                "D-Bus daemon did not provide a bus address".to_string(),
            ));
        }
        Ok((
            Self {
                child,
                _stdout: reader.into_inner(),
                _socket_dir: socket_dir,
            },
            address,
        ))
    }

    fn shutdown(&mut self) -> Result<()> {
        terminate_child(&mut self.child, "dbus-daemon")
    }
}

impl Drop for DbusDaemon {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            warn!(%error, "Failed to stop private OpenThread D-Bus daemon");
        }
    }
}

fn terminate_child(child: &mut Child, name: &str) -> Result<()> {
    if child
        .try_wait()
        .map_err(|source| OpenThreadError::FileSystem {
            operation: "inspect child process",
            path: PathBuf::from(name),
            source,
        })?
        .is_some()
    {
        return Ok(());
    }
    let pid = i32::try_from(child.id()).map_err(|_| {
        OpenThreadError::Process(format!("{name} returned an invalid process identifier"))
    })?;
    match kill(Pid::from_raw(-pid), Signal::SIGTERM) {
        Ok(()) => {}
        Err(Errno::ESRCH) => {
            child.wait().map_err(|source| OpenThreadError::FileSystem {
                operation: "reap child process after concurrent exit",
                path: PathBuf::from(name),
                source,
            })?;
            return Ok(());
        }
        Err(error) => {
            return Err(OpenThreadError::Process(format!(
                "failed to send SIGTERM to {name} process group: {error}"
            )));
        }
    }
    let deadline = Instant::now() + SHUTDOWN_GRACE;
    while Instant::now() < deadline {
        if child
            .try_wait()
            .map_err(|source| OpenThreadError::FileSystem {
                operation: "wait for child process shutdown",
                path: PathBuf::from(name),
                source,
            })?
            .is_some()
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    match kill(Pid::from_raw(-pid), Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => {
            return Err(OpenThreadError::Process(format!(
                "failed to force-stop {name} process group: {error}"
            )));
        }
    }
    child.wait().map_err(|source| OpenThreadError::FileSystem {
        operation: "reap child process",
        path: PathBuf::from(name),
        source,
    })?;
    Ok(())
}

fn generate_rest_token() -> Result<SecretString> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|error| {
        OpenThreadError::Process(format!("failed to generate OTBR REST credential: {error}"))
    })?;
    let mut token = String::with_capacity(random.len() * 2);
    for byte in random {
        write!(token, "{byte:02x}").expect("writing to a String cannot fail");
    }
    random.zeroize();
    Ok(SecretString::new(token))
}

fn validate_config(config: &BorderRouterConfig) -> Result<()> {
    if config.baud_rate == 0 {
        return Err(OpenThreadError::InvalidConfiguration(
            "RCP baud rate must be greater than zero".to_string(),
        ));
    }
    if config.thread_interface.is_empty()
        || !config
            .thread_interface
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(OpenThreadError::InvalidConfiguration(
            "Thread interface must contain only ASCII letters, digits, and underscores".to_string(),
        ));
    }
    if config.infrastructure_interface.trim().is_empty() {
        return Err(OpenThreadError::InvalidConfiguration(
            "infrastructure interface must not be empty".to_string(),
        ));
    }
    executable_if_present(&config.agent_path)?;
    fs::create_dir_all(&config.data_path).map_err(|source| OpenThreadError::FileSystem {
        operation: "create OpenThread data directory",
        path: config.data_path.clone(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&config.data_path, fs::Permissions::from_mode(0o700)).map_err(
            |source| OpenThreadError::FileSystem {
                operation: "secure OpenThread data directory",
                path: config.data_path.clone(),
                source,
            },
        )?;
    }
    if !config.rcp_device.exists() {
        return Err(OpenThreadError::InvalidConfiguration(format!(
            "RCP device does not exist: {}",
            config.rcp_device.display()
        )));
    }
    if !config
        .rcp_device
        .metadata()
        .map_err(|source| OpenThreadError::FileSystem {
            operation: "inspect OpenThread RCP device",
            path: config.rcp_device.clone(),
            source,
        })?
        .file_type()
        .is_char_device()
    {
        return Err(OpenThreadError::InvalidConfiguration(format!(
            "RCP path is not a character device: {}",
            config.rcp_device.display()
        )));
    }
    Ok(())
}

struct LogCapture {
    workers: Vec<JoinHandle<()>>,
}

impl LogCapture {
    fn start(
        path: PathBuf,
        stdout: impl Read + Send + 'static,
        stderr: impl Read + Send + 'static,
    ) -> Result<Self> {
        let writer = Arc::new(Mutex::new(RotatingLog::open(path)?));
        let workers = [
            ("stdout", Box::new(stdout) as Box<dyn Read + Send>),
            ("stderr", Box::new(stderr) as Box<dyn Read + Send>),
        ]
        .into_iter()
        .map(|(stream, reader)| {
            let writer = writer.clone();
            std::thread::spawn(move || copy_log(stream, reader, writer))
        })
        .collect();
        Ok(Self { workers })
    }

    fn finish(self) {
        for worker in self.workers {
            let _ = worker.join();
        }
    }
}

fn copy_log(
    stream: &'static str,
    mut reader: Box<dyn Read + Send>,
    writer: Arc<Mutex<RotatingLog>>,
) {
    let mut buffer = [0_u8; 8192];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(error) => {
                warn!(%error, stream, "Stopped capturing an OpenThread process log stream");
                return;
            }
        };
        if read == 0 {
            return;
        }
        let mut writer = match writer.lock() {
            Ok(writer) => writer,
            Err(_) => {
                warn!(
                    stream,
                    "Stopped capturing OpenThread logs because the writer lock was poisoned"
                );
                return;
            }
        };
        if let Err(error) = writer.write_all(&buffer[..read]) {
            warn!(%error, stream, "Stopped capturing OpenThread logs after a file write failure");
            return;
        }
    }
}

struct RotatingLog {
    path: PathBuf,
    file: fs::File,
    length: u64,
}

impl RotatingLog {
    fn open(path: PathBuf) -> Result<Self> {
        let length = path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        let file = open_log_file(&path, true).map_err(|source| OpenThreadError::FileSystem {
            operation: "open OTBR log",
            path: path.clone(),
            source,
        })?;
        Ok(Self { path, file, length })
    }

    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if self.length.saturating_add(bytes.len() as u64) > LOG_MAX_BYTES {
            let rotated = self.path.with_extension("log.1");
            match fs::remove_file(&rotated) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            fs::rename(&self.path, rotated)?;
            self.file = open_log_file(&self.path, false)?;
            self.length = 0;
        }
        self.file.write_all(bytes)?;
        self.file.flush()?;
        self.length = self.length.saturating_add(bytes.len() as u64);
        Ok(())
    }
}

fn open_log_file(path: &Path, append: bool) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options
        .create(true)
        .append(append)
        .write(!append)
        .truncate(!append);
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

fn startup_log_tail(log_path: &Path) -> String {
    let Ok(log) = fs::read_to_string(log_path) else {
        return "no agent diagnostics were captured".to_string();
    };
    let tail = log
        .lines()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ");
    if tail.is_empty() {
        "no agent diagnostics were captured".to_string()
    } else {
        tail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_uses_private_dynamic_rest_endpoint() {
        let config = BorderRouterConfig {
            agent_path: "/opt/extrittio/libexec/otbr-agent".into(),
            rcp_device: "/dev/ttyACM0".into(),
            baud_rate: 460_800,
            thread_interface: "wpan0".to_string(),
            infrastructure_interface: "en0".to_string(),
            data_path: "/tmp/extrittio-thread-test".into(),
        };
        let args = config.agent_args(39_841).unwrap();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--rest-listen-address", "127.0.0.1"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--rest-listen-port", "39841"])
        );
        assert_eq!(
            args.last().unwrap(),
            "spinel+hdlc+uart:///dev/ttyACM0?uart-baudrate=460800"
        );
    }

    #[test]
    fn agent_can_delegate_ephemeral_port_selection_to_otbr() {
        let config = BorderRouterConfig {
            agent_path: "/opt/extrittio/libexec/otbr-agent".into(),
            rcp_device: "/dev/ttyACM0".into(),
            baud_rate: 460_800,
            thread_interface: "wpan0".to_string(),
            infrastructure_interface: "en0".to_string(),
            data_path: "/tmp/extrittio-thread-test".into(),
        };
        let args = config.agent_args(0).unwrap();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--rest-listen-port", "0"])
        );
    }

    #[test]
    fn radio_url_rejects_query_injection_from_device_path() {
        let config = BorderRouterConfig {
            agent_path: "/opt/extrittio/libexec/otbr-agent".into(),
            rcp_device: "/dev/ttyACM0?uart-baudrate=1".into(),
            baud_rate: 460_800,
            thread_interface: "wpan0".to_string(),
            infrastructure_interface: "en0".to_string(),
            data_path: "/tmp/extrittio-thread-test".into(),
        };
        assert!(config.agent_args(39_841).is_err());
    }

    #[test]
    fn rest_credentials_are_high_entropy_and_not_debuggable() {
        let first = generate_rest_token().unwrap();
        let second = generate_rest_token().unwrap();
        assert_eq!(first.expose_secret().len(), 64);
        assert_ne!(first.expose_secret(), second.expose_secret());
        assert!(
            first
                .expose_secret()
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        );
        assert!(!format!("{first:?}").contains(first.expose_secret()));
    }

    #[test]
    fn rotating_log_bounds_the_active_file() {
        let path = std::env::temp_dir().join(format!("extrittio-otbr-log-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("log.1"));
        let mut log = RotatingLog::open(path.clone()).unwrap();
        log.write_all(&vec![b'a'; LOG_MAX_BYTES as usize]).unwrap();
        log.write_all(b"rotated").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"rotated");
        assert!(path.with_extension("log.1").metadata().unwrap().len() <= LOG_MAX_BYTES);
        fs::remove_file(&path).unwrap();
        fs::remove_file(path.with_extension("log.1")).unwrap();
    }

    #[test]
    fn process_exit_during_startup_is_reported_without_waiting_for_timeout() {
        let directory = tempfile::tempdir().unwrap();
        let log_path = directory.path().join("otbr-agent.log");
        fs::write(&log_path, "startup failed\n").unwrap();
        let child = Command::new("/bin/sh")
            .args(["-c", "exit 23"])
            .process_group(0)
            .spawn()
            .unwrap();
        let mut router = BorderRouter {
            child,
            config: BorderRouterConfig {
                agent_path: "/bin/sh".into(),
                rcp_device: "/dev/null".into(),
                baud_rate: 460_800,
                thread_interface: "wpan0".to_string(),
                infrastructure_interface: "en0".to_string(),
                data_path: directory.path().to_path_buf(),
            },
            dbus_daemon: None,
            log_capture: None,
            last_exit: None,
        };

        let started = Instant::now();
        let error = router
            .wait_for_rest_port(&directory.path().join("rest-port"), &log_path)
            .unwrap_err()
            .to_string();

        assert!(error.contains("before reporting its REST port"));
        assert!(error.contains("startup failed"));
        assert!(started.elapsed() < REST_PORT_TIMEOUT);
    }
}
