pub mod dashboard;
pub mod device_types;
pub mod devices;
pub mod fleets;
pub mod telemetry;

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(devices::router())
        .merge(dashboard::router())
        .merge(telemetry::router())
        .merge(device_types::router())
        .merge(fleets::router())
}
