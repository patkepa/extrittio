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
    observed_at: Instant,
}

/// Reports real CoreLocation observations only; unavailable location stays absent.
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
    pub fn latest(&self, max_age: Duration) -> Option<LocationData> {
        self.state
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .filter(|value| value.observed_at.elapsed() <= max_age)
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

            let age_seconds = -unsafe { location.timestamp() }.timeIntervalSinceNow();
            if !age_seconds.is_finite() || age_seconds < 0.0 { return; }
            let Ok(age) = Duration::try_from_secs_f64(age_seconds) else { return; };
            let Some(observed_at) = Instant::now().checked_sub(age) else { return; };
            let coord = unsafe { location.coordinate() };
            let altitude = unsafe { location.altitude() } as f32;
            let raw_speed = unsafe { location.speed() } as f32;
            let raw_heading = unsafe { location.course() } as f32;

            // CoreLocation returns -1.0 when speed/heading are unavailable.
            let speed = if raw_speed < 0.0 { 0.0 } else { raw_speed };
            let heading = if raw_heading < 0.0 { 0.0 } else { raw_heading };

            let data = LocationData {
                observed_at,
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
            tracing::warn!("Location permission denied/restricted — location is unavailable");
            manager.setDelegate(None);
            return;
        }

        if auth == CLAuthorizationStatus(0) {
            // NotDetermined — request authorization
            manager.requestAlwaysAuthorization();
        }

        tracing::info!("Starting CoreLocation updates...");
        manager.startUpdatingLocation();
    }

    let run_loop = NSRunLoop::currentRunLoop();
    loop {
        let future = NSDate::dateWithTimeIntervalSinceNow(1.0);
        run_loop.runUntilDate(&future);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_stale_fixes_are_absent_but_origin_is_valid() {
        let state = Arc::new(Mutex::new(None));
        let provider = LocationProvider {
            state: state.clone(),
        };
        assert!(provider.latest(Duration::from_secs(30)).is_none());
        *state.lock().unwrap() = Some(LocationData {
            latitude: 0.0,
            longitude: 0.0,
            altitude: 0.0,
            speed: 0.0,
            heading: 0.0,
            observed_at: Instant::now(),
        });
        assert_eq!(
            provider.latest(Duration::from_secs(30)).unwrap().latitude,
            0.0
        );
        state.lock().unwrap().as_mut().unwrap().observed_at =
            Instant::now() - Duration::from_secs(60);
        assert!(provider.latest(Duration::from_secs(30)).is_none());
    }
}
