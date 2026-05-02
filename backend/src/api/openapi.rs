use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
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
        // Auth
        super::auth_routes::login,
        super::auth_routes::me,
        // Dashboard
        super::dashboard::get_stats,
        // Users
        super::users::list_users,
        super::users::create_user,
        super::users::delete_user,
        super::users::change_password,
        // Devices
        super::devices::list_devices,
        super::devices::get_device,
        super::devices::create_device,
        super::devices::update_device,
        super::devices::delete_device,
        super::devices::restart_device,
        super::devices::trigger_ota,
        super::devices::list_ota_deployments,
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
        // Commands
        super::commands::send_command,
        super::commands::list_commands,
        // Firmware
        super::firmware_updates::list_firmware_updates,
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
        // Dashboard
        super::dashboard::DashboardStats,
        // Users
        super::users::CreateUserRequest,
        super::users::ChangePasswordRequest,
        // Devices
        super::devices::DeviceResponse,
        super::devices::NewDeviceRequest,
        super::devices::UpdateDeviceRequest,
        super::devices::TriggerOtaRequest,
        super::devices::OtaDeploymentResponse,
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
        // Commands
        super::commands::SendCommandRequest,
        super::commands::CommandResponse,
        // Firmware
        super::firmware_updates::FirmwareUpdateResponse,
        super::firmware_updates::NewFirmwareUpdateRequest,
        super::firmware_updates::NextVersionResponse,
        // Logs
        super::logs::LogResponse,
        // Config
        super::configs::ConfigResponse,
        // Certificates
        super::certificates::CaCertificateResponse,
        super::certificates::DeviceCertificateResponse,
        super::certificates::DeviceCertificateStatusResponse,
        // Health
        super::health::HealthResponse,
        super::health::ReadyResponse,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Health and readiness probes"),
        (name = "auth", description = "Authentication"),
        (name = "dashboard", description = "Dashboard statistics"),
        (name = "users", description = "User management"),
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
                        .build(),
                ),
            );
        }
    }
}
