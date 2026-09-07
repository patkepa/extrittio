use esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};

static STORE: OnceLock<Mutex<EspDefaultNvs>> = OnceLock::new();
static HEALTHY: AtomicBool = AtomicBool::new(false);

pub fn init(partition: EspDefaultNvsPartition) {
    let nvs = EspDefaultNvs::new(partition, "extrittio_ota", true).expect("OTA NVS");
    assert!(STORE.set(Mutex::new(nvs)).is_ok());
    if read().is_some_and(|state| state["phase"] == "pending") {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(120));
            if !HEALTHY.load(Ordering::SeqCst) {
                esp_idf_svc::hal::reset::restart();
            }
        });
    }
}
fn read() -> Option<Value> {
    let nvs = STORE.get()?.lock().ok()?;
    let mut buf = [0u8; 2048];
    serde_json::from_str(nvs.get_str("attempt", &mut buf).ok()??).ok()
}
pub fn stage(payload: &Value, target: &str) -> Result<(), esp_idf_svc::sys::EspError> {
    STORE.get().expect("OTA NVS").lock().unwrap().set_str(
        "attempt",
        &json!({"payload":payload,"target":target,"phase":"pending"}).to_string(),
    )
}
pub fn installed_version() -> Option<String> {
    let state = read()?;
    let ota = esp_idf_svc::ota::EspOta::new().ok()?;
    if state["target"].as_str() != Some(ota.get_running_slot().ok()?.label.as_str()) {
        return None;
    }
    state["payload"]["firmware_version"]
        .as_str()
        .map(|v| format!("v{}", v.trim_start_matches('v')))
}
pub fn same_attempt(id: i64) -> bool {
    read().is_some_and(|state| state["payload"]["deployment_id"].as_i64() == Some(id))
}
pub fn healthy() -> bool {
    HEALTHY.load(Ordering::SeqCst)
}
pub fn confirm() -> Result<Option<Value>, esp_idf_svc::sys::EspError> {
    let mut ota = esp_idf_svc::ota::EspOta::new()?;
    if ota.get_running_slot()?.state == embedded_svc::ota::SlotState::Unverified {
        ota.mark_running_slot_valid()?;
    }
    HEALTHY.store(true, Ordering::SeqCst);
    let Some(mut state) = read() else {
        return Ok(None);
    };
    let success = state["target"].as_str() == Some(ota.get_running_slot()?.label.as_str());
    state["phase"] = json!(if success { "confirmed" } else { "rolled_back" });
    STORE
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .set_str("attempt", &state.to_string())?;
    let Some(payload) = extrittio_sdk::ota::OtaPayload::from_json(&state["payload"]) else {
        return Ok(None);
    };
    Ok(Some(extrittio_sdk::ota::build_status_json(
        if success { "success" } else { "failed" },
        &payload.firmware_version,
        payload.firmware_update_id,
        payload.deployment_id,
        (!success).then_some("New image failed to boot; rolled back"),
    )))
}
