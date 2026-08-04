use utoipa::openapi::{
    path::Operation,
    security::{
        ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityRequirement, SecurityScheme,
    },
};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Extrittio API",
        description = "IoT Hub platform REST API",
        version = "1.0.0",
    ),
    paths(
        // Health
        super::health::health,
        super::health::ready,
        // System
        super::system::get_version,
        // Auth
        super::auth_routes::login,
        super::auth_routes::logout,
        super::auth_routes::me,
        // Dashboard
        super::dashboard::get_stats,
        // Audit
        super::audit::list_audit_events,
        // Users
        super::users::list_users,
        super::users::create_user,
        super::users::delete_user,
        super::users::change_password,
        super::users::set_roles,
        // Roles
        super::roles::list_roles,
        super::roles::list_permissions,
        super::roles::create_role,
        super::roles::update_role,
        super::roles::delete_role,
        // Devices
        super::devices::list_devices,
        super::devices::get_device,
        super::devices::create_device,
        super::devices::update_device,
        super::devices::delete_device,
        super::devices::restart_device,
        super::devices::trigger_ota,
        super::devices::list_ota_deployments,
        super::devices::bulk_change_fleet,
        super::devices::bulk_delete_devices,
        super::devices::bulk_restart_devices,
        super::devices::bulk_trigger_ota,
        super::devices::get_device_latest_location,
        // Device types
        super::device_types::list_device_types,
        super::device_types::create_device_type,
        super::device_types::update_device_type,
        super::device_types::delete_device_type,
        // Fleets
        super::fleets::list_fleets,
        super::fleets::create_fleet,
        super::fleets::update_fleet,
        super::fleets::delete_fleet,
        // Shadows
        super::shadows::get_shadow,
        super::shadows::update_desired,
        super::shadows::update_reported,
        super::shadows::delete_shadow,
        // Telemetry
        super::telemetry::get_device_telemetry,
        super::telemetry::get_latest_device_telemetry,
        super::telemetry::get_hourly_device_telemetry,
        // Commands
        super::commands::send_command,
        super::commands::list_commands,
        // Firmware
        super::firmware_updates::list_firmware_updates,
        super::firmware_updates::list_all_ota_deployments,
        super::firmware_updates::create_firmware_update,
        super::firmware_updates::upload_firmware_update,
        super::firmware_updates::download_firmware_blob,
        super::firmware_updates::delete_firmware_update,
        super::firmware_updates::get_next_version,
        // Logs
        super::logs::get_device_logs,
        // Config
        super::configs::get_config,
        super::configs::update_config,
        // Certificates
        super::certificates::get_ca_certificate,
        super::certificates::get_device_certificate,
        super::certificates::regenerate_device_certificate,
        super::certificates::get_device_certificate_status,
        // API keys and CI ingestion
        super::api_keys::list_api_keys,
        super::api_keys::create_api_key,
        super::api_keys::delete_api_key,
        super::ci_pipeline::ci_ingest,
        // Alerts
        super::alerts::list_alerts,
        super::alerts::get_summary,
        super::alerts::get_alert,
        super::alerts::acknowledge_alert,
        super::alerts::resolve_alert_handler,
        super::alerts::reactivate_alert_handler,
        super::alerts::bulk_acknowledge,
        super::alerts::bulk_resolve,
        super::alerts::bulk_reactivate,
        // Rules
        super::rules::list_rules,
        super::rules::get_rule,
        super::rules::create_rule,
        super::rules::update_rule_handler,
        super::rules::delete_rule_handler,
        super::rules::toggle_rule,
        // Zones
        super::zones::list_zones,
        super::zones::get_zone,
        super::zones::create_zone,
        super::zones::update_zone,
        super::zones::delete_zone,
        // Server metrics
        super::server_metrics::get_current_metrics,
        super::server_metrics::get_metrics_history,
        // Outbox
        super::outbox::get_summary,
        super::outbox::list_dead_letters,
        super::outbox::replay_dead_letters,
    ),
    components(schemas(
        // Pagination
        crate::pagination::PaginationParams,
        // Error
        crate::error::ErrorBody,
        // Auth
        super::auth_routes::LoginRequest,
        super::auth_routes::LoginResponse,
        super::auth_routes::UserResponse,
        super::auth_routes::RoleSummary,
        // Dashboard
        super::dashboard::DashboardStats,
        // Audit
        super::audit::AuditEventResponse,
        super::audit::AuditEventListResponse,
        // Users
        super::users::CreateUserRequest,
        super::users::ChangePasswordRequest,
        super::users::SetUserRolesRequest,
        // Roles
        super::roles::RoleResponse,
        super::roles::PermissionResponse,
        super::roles::CreateRoleRequest,
        super::roles::UpdateRoleRequest,
        // Devices
        super::devices::DeviceResponse,
        super::devices::NewDeviceRequest,
        super::devices::UpdateDeviceRequest,
        super::devices::TriggerOtaRequest,
        super::devices::OtaDeploymentResponse,
        super::devices::BulkDeviceFilters,
        super::devices::BulkFleetRequest,
        super::devices::BulkDeviceRequest,
        super::devices::BulkOtaRequest,
        super::devices::BulkAffectedResponse,
        super::devices::BulkOperationError,
        super::devices::BulkResultResponse,
        super::devices::LocationResponse,
        // Device types
        super::device_types::DeviceTypeResponse,
        super::device_types::NewDeviceTypeRequest,
        super::device_types::UpdateDeviceTypeRequest,
        // Fleets
        super::fleets::FleetResponse,
        super::fleets::NewFleetRequest,
        super::fleets::UpdateFleetRequest,
        // Shadows
        super::shadows::ShadowResponse,
        // Telemetry
        super::telemetry::TelemetryResponse,
        super::telemetry::HourlyTelemetryResponse,
        // Commands
        super::commands::SendCommandRequest,
        super::commands::CommandResponse,
        // Firmware
        super::firmware_updates::FirmwareUpdateResponse,
        super::firmware_updates::NewFirmwareUpdateRequest,
        super::firmware_updates::NextVersionResponse,
        super::firmware_updates::GlobalOtaDeploymentResponse,
        // Logs
        super::logs::LogResponse,
        // Config
        super::configs::ConfigResponse,
        // Certificates
        super::certificates::CaCertificateResponse,
        super::certificates::DeviceCertificateResponse,
        super::certificates::DeviceCertificateStatusResponse,
        // API keys and CI ingestion
        super::api_keys::CreateApiKeyRequest,
        super::api_keys::CreateApiKeyResponse,
        super::api_keys::ApiKeyResponse,
        super::ci_pipeline::CiIngestRequest,
        super::ci_pipeline::CiIngestResponse,
        // Alerts
        super::alerts::BulkAlertIds,
        super::alerts::AlertResponse,
        super::alerts::AlertSummary,
        super::alerts::AlertSeverityCounts,
        // Rules
        super::rules::CreateRuleRequest,
        super::rules::UpdateRuleRequest,
        super::rules::ConditionInput,
        super::rules::ActionInput,
        super::rules::EnabledInput,
        super::rules::RuleResponse,
        super::rules::ConditionResponse,
        super::rules::ActionResponse,
        // Zones
        super::zones::CreateZoneRequest,
        super::zones::UpdateZoneRequest,
        super::zones::ZoneResponse,
        // Server metrics
        super::server_metrics::CurrentMetricsResponse,
        super::server_metrics::MetricsHistoryResponse,
        super::server_metrics::SystemMetricsSnapshot,
        super::server_metrics::AppMetricsSnapshot,
        // Health
        super::health::HealthResponse,
        super::health::ReadyResponse,
        // System
        super::system::SystemVersionResponse,
        // Outbox
        super::outbox::RuleActionOutboxSummaryResponse,
        super::outbox::DeadLetterEventResponse,
        super::outbox::DeadLetterListResponse,
        super::outbox::ReplayDeadLettersRequest,
        super::outbox::ReplayDeadLettersResponse,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Health and readiness probes"),
        (name = "auth", description = "Authentication"),
        (name = "api-keys", description = "API key management"),
        (name = "alerts", description = "Alert operations"),
        (name = "rules", description = "Rule engine configuration"),
        (name = "zones", description = "Geofence zone management"),
        (name = "dashboard", description = "Dashboard statistics"),
        (name = "audit", description = "Security and administrative audit trail"),
        (name = "users", description = "User management"),
        (name = "roles", description = "Role and permission management"),
        (name = "devices", description = "Device management"),
        (name = "device-types", description = "Device type management"),
        (name = "fleets", description = "Fleet management"),
        (name = "shadows", description = "Device shadow (desired/reported state)"),
        (name = "telemetry", description = "Device telemetry data"),
        (name = "commands", description = "Device commands"),
        (name = "firmware", description = "Firmware updates and OTA"),
        (name = "logs", description = "Device logs"),
        (name = "config", description = "Device configuration"),
        (name = "certificates", description = "TLS certificates"),
        (name = "server-metrics", description = "Server metrics and operational queues"),
        (name = "system", description = "Runtime system metadata"),
    ),
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(ref mut components) = openapi.components {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some(
                            "JWT bearer authentication for explicitly enabled non-browser clients",
                        ))
                        .build(),
                ),
            );
            components.add_security_scheme(
                "session_cookie",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                    "extrittio_session",
                    "HTTP-only browser session cookie issued by POST /api/v1/auth/login",
                ))),
            );
        }

        for path_item in openapi.paths.paths.values_mut() {
            add_session_cookie_alternative(path_item.get.as_mut());
            add_session_cookie_alternative(path_item.put.as_mut());
            add_session_cookie_alternative(path_item.post.as_mut());
            add_session_cookie_alternative(path_item.delete.as_mut());
            add_session_cookie_alternative(path_item.options.as_mut());
            add_session_cookie_alternative(path_item.head.as_mut());
            add_session_cookie_alternative(path_item.patch.as_mut());
            add_session_cookie_alternative(path_item.trace.as_mut());
        }
    }
}

fn add_session_cookie_alternative(operation: Option<&mut Operation>) {
    let Some(security) = operation.and_then(|operation| operation.security.as_mut()) else {
        return;
    };

    let bearer = SecurityRequirement::new("bearer_auth", Vec::<String>::new());
    let session_cookie = SecurityRequirement::new("session_cookie", Vec::<String>::new());

    if security.contains(&bearer) && !security.contains(&session_cookie) {
        security.push(session_cookie);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_operations_document_cookie_or_bearer_authentication() {
        let document = ApiDoc::openapi();
        let operation = document.paths.paths["/api/v1/devices"]
            .get
            .as_ref()
            .expect("device list operation");
        let security = operation.security.as_ref().expect("security requirements");

        assert!(security.contains(&SecurityRequirement::new(
            "bearer_auth",
            Vec::<String>::new()
        )));
        assert!(security.contains(&SecurityRequirement::new(
            "session_cookie",
            Vec::<String>::new()
        )));
    }

    #[test]
    fn public_operations_remain_public() {
        let document = ApiDoc::openapi();
        let login = document.paths.paths["/api/v1/auth/login"]
            .post
            .as_ref()
            .expect("login operation");
        let health = document.paths.paths["/health"]
            .get
            .as_ref()
            .expect("health operation");

        assert!(login.security.is_none());
        assert!(health.security.is_none());
    }
}
