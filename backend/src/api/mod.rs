pub mod auth_routes;
pub mod commands;
pub mod configs;
pub mod dashboard;
pub mod device_types;
pub mod devices;
pub mod firmware_updates;
pub mod fleets;
pub mod logs;
pub mod shadows;
pub mod telemetry;
pub mod users;

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(auth_routes::router())
        .merge(devices::router())
        .merge(dashboard::router())
        .merge(telemetry::router())
        .merge(device_types::router())
        .merge(fleets::router())
        .merge(shadows::router())
        .merge(users::router())
        .merge(firmware_updates::router())
        .merge(logs::router())
        .merge(configs::router())
        .merge(commands::router())
}
