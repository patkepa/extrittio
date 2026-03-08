pub mod dashboard;
pub mod devices;
pub mod telemetry;

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(devices::router())
        .merge(dashboard::router())
        .merge(telemetry::router())
}
