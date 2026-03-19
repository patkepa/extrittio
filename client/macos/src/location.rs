use std::sync::{Arc, Mutex};

/// Real-time location data from CoreLocation.
#[derive(Debug, Clone, Copy)]
pub struct LocationData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f32,
    pub speed: f32,
    pub heading: f32,
}

/// Provides real device location via macOS CoreLocation.
///
/// Spawns a background thread running an NSRunLoop for delegate callbacks.
/// The telemetry loop reads the latest location via `latest()`.
/// Falls back gracefully: `latest()` returns `None` if location is
/// unavailable, permission is denied, or no fix has been obtained yet.
pub struct LocationProvider {
    state: Arc<Mutex<Option<LocationData>>>,
}

impl LocationProvider {
    /// Create a new provider. Spawns a background CoreLocation thread.
    pub fn new() -> Self {
        let state: Arc<Mutex<Option<LocationData>>> = Arc::new(Mutex::new(None));

        let writer = state.clone();
        std::thread::Builder::new()
            .name("core-location".into())
            .spawn(move || {
                run_location_loop(writer);
            })
            .expect("Failed to spawn CoreLocation thread");

        Self { state }
    }

    /// Read the most recent location. Non-blocking.
    /// Returns `None` if no location has been received yet.
    pub fn latest(&self) -> Option<LocationData> {
        self.state.lock().ok().and_then(|guard| *guard)
    }
}

fn run_location_loop(_state: Arc<Mutex<Option<LocationData>>>) {
    // Will be implemented in the next task.
    // For now, just log and return (thread exits immediately).
    // This prevents compilation errors while we build incrementally.
    tracing::warn!("CoreLocation loop not yet implemented");
}
