pub mod alerts;
pub mod api_keys;
pub mod auth_routes;
pub mod certificates;
pub mod ci_pipeline;
pub mod commands;
pub mod configs;
pub mod dashboard;
pub mod device_types;
pub mod devices;
pub mod firmware_updates;
pub mod fleets;
pub mod health;
pub mod logs;
pub mod openapi;
pub mod outbox;
pub mod roles;
pub mod rules;
pub mod server_metrics;
pub mod shadows;
pub mod system;
pub mod telemetry;
pub mod users;
pub mod zones;

use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::state::AppState;

pub fn router(max_firmware_size: usize) -> Router<Arc<AppState>> {
    Router::new()
        .merge(api_keys::router())
        .merge(auth_routes::router())
        .merge(ci_pipeline::router())
        .merge(devices::router())
        .merge(dashboard::router())
        .merge(telemetry::router())
        .merge(device_types::router())
        .merge(fleets::router())
        .merge(shadows::router())
        .merge(users::router())
        .merge(firmware_updates::router(max_firmware_size))
        .merge(logs::router())
        .merge(configs::router())
        .merge(commands::router())
        .merge(certificates::router())
        .merge(roles::router())
        .merge(rules::router())
        .merge(alerts::router())
        .merge(health::router())
        .merge(server_metrics::router())
        .merge(outbox::router())
        .merge(system::router())
        .merge(zones::router())
        .merge(
            SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi::ApiDoc::openapi()),
        )
}
