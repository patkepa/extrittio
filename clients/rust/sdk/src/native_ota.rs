//! Durable native installation state and an independent rollback watchdog.
//! Process supervisors must restart the client after a successful exit.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

fn sidecar(exe: &Path, suffix: &str) -> PathBuf {
    let mut name = exe.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}
fn lock(exe: &Path) -> io::Result<File> {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(sidecar(exe, ".ota-lock"))?;
    f.lock()?;
    Ok(f)
}
fn read(exe: &Path) -> io::Result<Value> {
    match fs::read(sidecar(exe, ".ota-state")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(e),
    }
}
fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let tmp = sidecar(path, &format!(".tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&tmp)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        File::open(
            path.parent()
                .ok_or_else(|| io::Error::other("Missing parent"))?,
        )?
        .sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
fn save(exe: &Path, state: &Value) -> io::Result<()> {
    atomic_write(
        &sidecar(exe, ".ota-state"),
        &serde_json::to_vec(state)?,
        0o600,
    )
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Preserve the running image before replacing it, and start its watchdog.
pub fn install(
    exe: &Path,
    bytes: &[u8],
    payload: &Value,
    previous_version: &str,
) -> io::Result<()> {
    let _guard = lock(exe)?;
    let parsed = crate::ota::OtaPayload::from_json(payload)
        .ok_or_else(|| io::Error::other("Invalid OTA manifest"))?;
    let previous = fs::read(exe)?;
    let backup = sidecar(exe, ".ota-previous");
    atomic_write(&backup, &previous, 0o755)?;
    let state = json!({"phase":"pending", "payload":payload, "digest":digest(bytes),
        "previous_digest":digest(&previous), "previous_version":previous_version,
        "version":format!("v{}", parsed.firmware_version.trim_start_matches('v'))});
    save(exe, &state)?;
    // The old, already running client monitors a candidate even if it cannot execute.
    Command::new(&backup)
        // launchd cleans up the exiting main process's group.
        .process_group(0)
        .env("EXTRITTIO_OTA_WATCHDOG", exe)
        .env("EXTRITTIO_OTA_ATTEMPT", parsed.deployment_id.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    atomic_write(exe, bytes, 0o755)
}

/// Call before configuration parsing or network startup in every native entry point.
pub fn watchdog_entry() {
    let Some(exe) = std::env::var_os("EXTRITTIO_OTA_WATCHDOG") else {
        return;
    };
    let attempt = std::env::var("EXTRITTIO_OTA_ATTEMPT")
        .ok()
        .and_then(|v| v.parse::<i64>().ok());
    std::thread::sleep(Duration::from_secs(120));
    let result = rollback(Path::new(&exe), attempt);
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}

fn rollback(exe: &Path, attempt: Option<i64>) -> io::Result<()> {
    let _guard = lock(exe)?;
    let mut state = read(exe)?;
    if attempt.is_none()
        || state["payload"]["deployment_id"].as_i64() != attempt
        || !matches!(state["phase"].as_str(), Some("pending" | "booting"))
    {
        return Ok(());
    }
    let previous = fs::read(sidecar(exe, ".ota-previous"))?;
    if state["previous_digest"].as_str() != Some(&digest(&previous)) {
        return Err(io::Error::other("Rollback image integrity failure"));
    }
    atomic_write(exe, &previous, 0o755)?;
    state["phase"] = json!("rolled_back");
    save(exe, &state)?;
    Ok(())
}

pub fn boot(exe: &Path) -> io::Result<Option<String>> {
    // Ordinary clients must not require a writable binary directory.
    if !sidecar(exe, ".ota-state").exists() {
        return Ok(None);
    }
    let _guard = lock(exe)?;
    let mut state = read(exe)?;
    let hash = digest(&fs::read(exe)?);
    if state["phase"] == "rolled_back" && state["previous_digest"] == hash {
        return Ok(state["previous_version"].as_str().map(str::to_owned));
    }
    if state["digest"] != hash {
        return Ok(None);
    }
    if matches!(state["phase"].as_str(), Some("pending" | "booting")) {
        state["phase"] = json!("booting");
        save(exe, &state)?;
        let executable = exe.to_owned();
        let attempt = state["payload"]["deployment_id"].clone();
        // Exit this exact candidate after the independent watchdog restores the
        // previous image. Avoid signalling a numeric PID that could be reused.
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(2));
                if let Ok(state) = read(&executable) {
                    if state["payload"]["deployment_id"] != attempt || state["phase"] == "confirmed"
                    {
                        break;
                    }
                    if state["phase"] == "rolled_back" {
                        std::process::exit(1);
                    }
                }
            }
        });
    }
    Ok(state["version"].as_str().map(str::to_owned))
}

pub fn is_installed_attempt(exe: &Path, deployment_id: i64) -> bool {
    read(exe).ok().is_some_and(|state| {
        state["payload"]["deployment_id"].as_i64() == Some(deployment_id)
            && fs::read(exe)
                .ok()
                .is_some_and(|bytes| state["digest"] == digest(&bytes))
    })
}

pub fn needs_confirmation(exe: &Path) -> bool {
    read(exe)
        .ok()
        .is_some_and(|state| state["phase"] == "booting")
}

/// Called after the client has maintained its session and telemetry for 30s.
/// The returned terminal report is retained and may be resent after reconnect.
pub fn confirm(exe: &Path) -> io::Result<Option<Value>> {
    if !sidecar(exe, ".ota-state").exists() {
        return Ok(None);
    }
    let _guard = lock(exe)?;
    let mut state = read(exe)?;
    if state["phase"] == "confirmed" && state["digest"] != digest(&fs::read(exe)?) {
        return Ok(None);
    }
    if state["phase"] == "booting" {
        if state["digest"] != digest(&fs::read(exe)?) {
            return Err(io::Error::other("Installed image changed"));
        }
        state["phase"] = json!("confirmed");
        save(exe, &state)?;
    }
    let status = match state["phase"].as_str() {
        Some("confirmed") => "success",
        Some("rolled_back") => "failed",
        _ => return Ok(None),
    };
    let payload = crate::ota::OtaPayload::from_json(&state["payload"])
        .ok_or_else(|| io::Error::other("Invalid installation journal"))?;
    Ok(Some(crate::ota::build_status_json(
        status,
        &payload.firmware_version,
        payload.firmware_update_id,
        payload.deployment_id,
        (status == "failed").then_some("Startup health check failed; previous image restored"),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("device");
        fs::write(&exe, b"new-image").unwrap();
        fs::write(sidecar(&exe, ".ota-previous"), b"previous-image").unwrap();
        save(&exe, &json!({"phase":"pending", "digest":digest(b"new-image"),
            "previous_digest":digest(b"previous-image"), "previous_version":"v1.0.0", "version":"v2.0.0",
            "payload":{"firmware_version":"2.0.0", "firmware_url":"https://example.com/image",
                "firmware_update_id":42, "deployment_id":7, "sha256":"a".repeat(64)}})).unwrap();
        (dir, exe)
    }
    #[test]
    fn pending_image_is_not_successful_until_boot_confirmation() {
        let (_dir, exe) = fixture();
        assert!(confirm(&exe).unwrap().is_none());
        let mut state = read(&exe).unwrap();
        state["phase"] = json!("booting");
        save(&exe, &state).unwrap();
        let report = confirm(&exe).unwrap().unwrap();
        assert_eq!(report["status"], "success");
        assert_eq!(report["deployment_id"], 7);
        assert_eq!(boot(&exe).unwrap().as_deref(), Some("v2.0.0"));
        rollback(&exe, Some(7)).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"new-image");
    }
    #[test]
    fn failed_boot_restores_previous_image_and_reports_matching_attempt() {
        let (_dir, exe) = fixture();
        rollback(&exe, Some(6)).unwrap(); // An old watchdog cannot undo a later attempt.
        assert_eq!(fs::read(&exe).unwrap(), b"new-image");
        rollback(&exe, Some(7)).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"previous-image");
        assert_eq!(boot(&exe).unwrap().as_deref(), Some("v1.0.0"));
        let report = confirm(&exe).unwrap().unwrap();
        assert_eq!(report["status"], "failed");
        assert_eq!(report["deployment_id"], 7);
    }
    #[test]
    fn corrupted_backup_is_never_activated() {
        let (_dir, exe) = fixture();
        fs::write(sidecar(&exe, ".ota-previous"), b"corrupt").unwrap();
        assert!(rollback(&exe, Some(7)).is_err());
        assert_eq!(fs::read(&exe).unwrap(), b"new-image");
    }
}
