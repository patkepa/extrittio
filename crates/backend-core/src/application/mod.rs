mod zones;

use std::sync::Arc;

use crate::ZoneRepository;

pub use zones::{CreateZone, ZoneApplication, ZoneUpdate};

/// Curated application façade passed to transports.
#[derive(Clone)]
pub struct Application {
    zones: ZoneApplication,
}

impl Application {
    #[must_use]
    pub fn new(zones: Arc<dyn ZoneRepository>) -> Self {
        Self {
            zones: ZoneApplication::new(zones),
        }
    }

    #[must_use]
    pub fn zones(&self) -> &ZoneApplication {
        &self.zones
    }
}
