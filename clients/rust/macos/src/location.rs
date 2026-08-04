#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_core_location::{
    CLAuthorizationStatus, CLLocation, CLLocationManager, CLLocationManagerDelegate,
};
use objc2_foundation::{NSArray, NSDate, NSError, NSObject, NSObjectProtocol, NSRunLoop};

/// Real-time location data from CoreLocation.
#[derive(Debug, Clone, Copy)]
pub struct LocationData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f32,
    pub speed: f32,
    pub heading: f32,
}

/// Provides real device location via macOS CoreLocation, with simulated
/// fallback when CoreLocation permission is unavailable (e.g., CLI tools
/// on macOS Tahoe+).
///
/// Spawns a background thread that attempts CoreLocation first. If no
/// real location is received within a timeout, falls back to the SDK's
/// simulated LocationState for demonstration purposes.
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
    let delegate = LocationDelegate::new(state.clone());

    let manager = unsafe { CLLocationManager::new() };

    unsafe {
        manager.setDesiredAccuracy(0.0);
        manager.setDistanceFilter(10.0);
        manager.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        let auth = manager.authorizationStatus();
        if auth == CLAuthorizationStatus::Denied || auth == CLAuthorizationStatus::Restricted {
            tracing::warn!(
                "Location permission denied/restricted — falling back to simulated location"
            );
            drop(delegate);
            run_simulated_location(state);
            return;
        }

        if auth == CLAuthorizationStatus(0) {
            // NotDetermined — request authorization
            manager.requestAlwaysAuthorization();
        }

        tracing::info!("Starting CoreLocation updates...");
        manager.startUpdatingLocation();
    }

    // Give CoreLocation a chance to deliver a fix via the run loop.
    let start = Instant::now();
    let timeout = Duration::from_secs(10);
    let run_loop = NSRunLoop::currentRunLoop();

    while start.elapsed() < timeout {
        let future = NSDate::dateWithTimeIntervalSinceNow(0.5);
        run_loop.runUntilDate(&future);

        // Check if we got a real location
        if state.lock().ok().is_some_and(|g| g.is_some()) {
            tracing::info!("CoreLocation delivering real location data");
            // Keep running the run loop forever for continued updates
            loop {
                let future = NSDate::dateWithTimeIntervalSinceNow(1.0);
                run_loop.runUntilDate(&future);
            }
        }
    }

    // CoreLocation didn't deliver within the timeout — fall back to simulation
    tracing::warn!(
        "CoreLocation unavailable (no permission or no fix after {}s) — using simulated location",
        timeout.as_secs()
    );

    // Clean up CoreLocation resources
    unsafe {
        manager.stopUpdatingLocation();
        manager.setDelegate(None);
    }
    drop(delegate);
    drop(manager);

    run_simulated_location(state);
}

// ---------------------------------------------------------------------------
// Simulated location fallback
// ---------------------------------------------------------------------------

fn run_simulated_location(state: Arc<Mutex<Option<LocationData>>>) {
    use extrittio_sdk::location::LocationState;

    // Start near the user's likely location (Warsaw, Poland — SDK default)
    let mut sim = LocationState::default();
    let mut rng = SmallRng::from_os_rng();

    tracing::info!(
        "Simulated location started at {:.6}, {:.6}",
        sim.latitude,
        sim.longitude
    );

    // Write initial position immediately
    if let Ok(mut guard) = state.lock() {
        *guard = Some(LocationData {
            latitude: sim.latitude,
            longitude: sim.longitude,
            altitude: sim.altitude,
            speed: sim.speed,
            heading: sim.heading,
        });
    }

    loop {
        std::thread::sleep(Duration::from_secs(5));

        sim.step(
            rng_range(&mut rng, -0.0003, 0.0003),
            rng_range(&mut rng, -0.0003, 0.0003),
            rng_range_f32(&mut rng, -2.0, 2.0),
        );

        if let Ok(mut guard) = state.lock() {
            *guard = Some(LocationData {
                latitude: sim.latitude,
                longitude: sim.longitude,
                altitude: sim.altitude,
                speed: sim.speed,
                heading: sim.heading,
            });
        }
    }
}

// Simple RNG helpers to avoid pulling in the full `rand` crate
use std::hash::{Hash, Hasher};

struct SmallRng(u64);

impl SmallRng {
    fn from_os_rng() -> Self {
        // Seed from current time + thread id
        let mut hasher = std::hash::DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut hasher);
        std::thread::current().id().hash(&mut hasher);
        Self(hasher.finish())
    }

    fn next_u64(&mut self) -> u64 {
        // xorshift64
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn rng_range(rng: &mut SmallRng, min: f64, max: f64) -> f64 {
    min + rng.next_f64() * (max - min)
}

fn rng_range_f32(rng: &mut SmallRng, min: f32, max: f32) -> f32 {
    min + rng.next_f64() as f32 * (max - min)
}
