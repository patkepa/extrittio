#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, AnyThread, DefinedClass};
use objc2_core_location::{
    CLAuthorizationStatus, CLLocation, CLLocationManager, CLLocationManagerDelegate,
};
use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol, NSRunLoop};

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

// ---------------------------------------------------------------------------
// Objective-C delegate class
// ---------------------------------------------------------------------------

struct DelegateIvars {
    state: Arc<Mutex<Option<LocationData>>>,
}

define_class! {
    #[unsafe(super(NSObject))]
    #[name = "ExtrittioLocationDelegate"]
    #[ivars = DelegateIvars]
    struct LocationDelegate;

    unsafe impl NSObjectProtocol for LocationDelegate {}

    unsafe impl CLLocationManagerDelegate for LocationDelegate {
        #[unsafe(method(locationManager:didUpdateLocations:))]
        unsafe fn locationManager_didUpdateLocations(
            &self,
            _manager: &CLLocationManager,
            locations: &NSArray<CLLocation>,
        ) {
            let count = locations.count();
            if count == 0 {
                return;
            }

            // Use the most recent location (last in array).
            let location = locations.objectAtIndex(count - 1);

            // Check validity: negative horizontalAccuracy means invalid.
            let h_acc = unsafe { location.horizontalAccuracy() };
            if h_acc < 0.0 {
                tracing::debug!("Ignoring invalid location (horizontalAccuracy < 0)");
                return;
            }

            let coord = unsafe { location.coordinate() };
            let altitude = unsafe { location.altitude() } as f32;
            let raw_speed = unsafe { location.speed() } as f32;
            let raw_heading = unsafe { location.course() } as f32;

            // CoreLocation returns -1.0 when speed/heading are unavailable.
            let speed = if raw_speed < 0.0 { 0.0 } else { raw_speed };
            let heading = if raw_heading < 0.0 { 0.0 } else { raw_heading };

            let data = LocationData {
                latitude: coord.latitude,
                longitude: coord.longitude,
                altitude,
                speed,
                heading,
            };

            if let Ok(mut guard) = self.ivars().state.lock() {
                let first_fix: bool = guard.is_none();
                *guard = Some(data);
                drop(guard);

                if first_fix {
                    tracing::info!(
                        "Location acquired: {:.6}, {:.6} (accuracy: {:.0}m)",
                        data.latitude,
                        data.longitude,
                        h_acc,
                    );
                }
            }
        }

        #[unsafe(method(locationManager:didFailWithError:))]
        unsafe fn locationManager_didFailWithError(
            &self,
            _manager: &CLLocationManager,
            error: &NSError,
        ) {
            tracing::warn!("CoreLocation error: {:?}", error);
        }

        #[unsafe(method(locationManagerDidChangeAuthorization:))]
        unsafe fn locationManagerDidChangeAuthorization(
            &self,
            manager: &CLLocationManager,
        ) {
            let status = unsafe { manager.authorizationStatus() };
            match status {
                CLAuthorizationStatus::AuthorizedAlways => {
                    tracing::info!("Location permission granted");
                    unsafe { manager.startUpdatingLocation() };
                }
                CLAuthorizationStatus::Denied => {
                    tracing::warn!(
                        "Location permission denied — location will not be reported"
                    );
                }
                CLAuthorizationStatus::Restricted => {
                    tracing::warn!(
                        "Location access restricted — location will not be reported"
                    );
                }
                CLAuthorizationStatus::NotDetermined => {
                    tracing::info!("Location permission not yet determined, waiting for user...");
                }
                _ => {
                    tracing::info!("Location authorization status: {:?}", status);
                }
            }
        }
    }
}

impl LocationDelegate {
    fn new(state: Arc<Mutex<Option<LocationData>>>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { state });
        unsafe { msg_send![super(this), init] }
    }
}

// ---------------------------------------------------------------------------
// Background thread entry point
// ---------------------------------------------------------------------------

fn run_location_loop(state: Arc<Mutex<Option<LocationData>>>) {
    let delegate = LocationDelegate::new(state);

    // Create CLLocationManager in the outer scope so it stays alive
    // alongside the NSRunLoop. If it were inside an inner unsafe block,
    // the Retained<CLLocationManager> would be dropped when that block
    // ends, deallocating the manager before the run loop starts.
    let manager = unsafe { CLLocationManager::new() };

    unsafe {
        // desiredAccuracy = kCLLocationAccuracyBest (0.0)
        manager.setDesiredAccuracy(0.0);

        // distanceFilter = 10.0 meters
        manager.setDistanceFilter(10.0);

        // Set delegate
        manager.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        // Check authorization and start updates.
        // On macOS, calling startUpdatingLocation() triggers the permission
        // prompt if status is NotDetermined. The delegate's
        // locationManagerDidChangeAuthorization: callback handles the rest.
        let status = manager.authorizationStatus();
        if status == CLAuthorizationStatus::Denied
            || status == CLAuthorizationStatus::Restricted
        {
            tracing::warn!(
                "Location permission denied/restricted — skipping location updates"
            );
            return; // Thread exits; latest() will always return None.
        }

        tracing::info!("Starting CoreLocation updates...");
        manager.startUpdatingLocation();
    }

    // Run the NSRunLoop forever to receive delegate callbacks.
    // Both `manager` and `delegate` are alive on the stack here.
    // This thread is intentionally never stopped — it dies on process exit.
    NSRunLoop::currentRunLoop().run();
}
