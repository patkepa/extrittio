#[path = "../domains/alerts/alerts.rs"]
pub mod alerts;
#[path = "../domains/identity/api_keys.rs"]
pub mod api_keys;
#[path = "../domains/audit/audit.rs"]
pub mod audit;
#[path = "../domains/identity/auth_routes.rs"]
pub mod auth_routes;
#[path = "../domains/identity/certificates.rs"]
pub mod certificates;
#[path = "../domains/firmware/ci_pipeline.rs"]
pub mod ci_pipeline;
#[path = "../domains/commands/commands.rs"]
pub mod commands;
#[path = "../domains/configuration/configs.rs"]
pub mod configs;
#[path = "../domains/dashboard/dashboard.rs"]
pub mod dashboard;
#[path = "../domains/device_types/device_types.rs"]
pub mod device_types;
#[path = "../domains/devices/devices.rs"]
pub mod devices;
#[path = "../domains/firmware/firmware_updates.rs"]
pub mod firmware_updates;
#[path = "../domains/fleets/fleets.rs"]
pub mod fleets;
#[path = "../domains/operations/health.rs"]
pub mod health;
#[path = "../domains/logs/logs.rs"]
pub mod logs;
pub mod openapi;
#[path = "../domains/operations/outbox.rs"]
pub mod outbox;
#[path = "../domains/identity/roles.rs"]
pub mod roles;
#[path = "../domains/rules/rules.rs"]
pub mod rules;
#[path = "../domains/operations/server_metrics_api.rs"]
pub mod server_metrics;
#[path = "../domains/shadows/shadows.rs"]
pub mod shadows;
#[path = "../domains/operations/system.rs"]
pub mod system;
#[path = "../domains/telemetry/telemetry.rs"]
pub mod telemetry;
#[path = "../domains/operations/thread.rs"]
pub mod thread;
#[path = "../domains/identity/users.rs"]
pub mod users;
#[path = "../domains/zones/zones.rs"]
pub mod zones;

use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::state::AppState;

pub fn router(max_firmware_size: usize, enable_api_docs: bool) -> Router<Arc<AppState>> {
    let router = Router::new()
        .merge(api_keys::router())
        .merge(audit::router())
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
        .merge(thread::router())
        .merge(zones::router());

    if enable_api_docs {
        router.merge(
            SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi::ApiDoc::openapi()),
        )
    } else {
        router
    }
}
