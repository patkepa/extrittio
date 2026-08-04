import Foundation

enum Endpoints {
    static func baseURL(serverAddress: String) -> String {
        APIConfiguration.current.baseURL(serverAddress: serverAddress)?.absoluteString ?? ""
    }

    // Auth
    static func login(_ server: String) -> String { endpoint(server, "auth/login") }
    static func me(_ server: String) -> String { endpoint(server, "auth/me") }

    // Admin
    static func users(_ server: String) -> String { endpoint(server, "users") }
    static func user(_ server: String, id: Int) -> String { endpoint(server, "users/\(id)") }
    static func userRoles(_ server: String, id: Int) -> String { endpoint(server, "users/\(id)/roles") }
    static func userPassword(_ server: String, id: Int) -> String { endpoint(server, "users/\(id)/password") }
    static func roles(_ server: String) -> String { endpoint(server, "roles") }
    static func role(_ server: String, id: Int) -> String { endpoint(server, "roles/\(id)") }
    static func rolePermissions(_ server: String) -> String { endpoint(server, "roles/permissions") }
    static func apiKeys(_ server: String) -> String { endpoint(server, "api-keys") }
    static func apiKey(_ server: String, id: Int) -> String { endpoint(server, "api-keys/\(id)") }
    static func caCertificate(_ server: String) -> String { endpoint(server, "ca/certificate") }
    static func deviceCertificate(_ server: String, id: String) -> String { endpoint(server, "devices/\(id)/certificate") }
    static func deviceCertificateRegenerate(_ server: String, id: String) -> String { endpoint(server, "devices/\(id)/certificate/regenerate") }
    static func deviceCertificateStatus(_ server: String, id: String) -> String { endpoint(server, "devices/\(id)/certificate/status") }

    // Dashboard
    static func dashboardStats(_ server: String) -> String { endpoint(server, "dashboard/stats") }
    static func alertSummary(_ server: String) -> String { endpoint(server, "alerts/summary") }

    // MARK: - Server Metrics
    static func serverMetricsCurrent(_ server: String) -> String {
        endpoint(server, "server/metrics/current")
    }

    static func serverMetricsHistory(_ server: String, since: String, resolution: Int = 10) -> String {
        endpoint(server, "server/metrics/history", queryItems: [
            URLQueryItem(name: "since", value: since),
            URLQueryItem(name: "resolution", value: "\(resolution)")
        ])
    }

    // Devices
    static func devices(_ server: String) -> String { endpoint(server, "devices") }
    static func device(_ server: String, id: String) -> String { endpoint(server, "devices/\(id)") }
    static func deviceRestart(_ server: String, id: String) -> String { endpoint(server, "devices/\(id)/restart") }

    // Telemetry
    static func telemetry(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/telemetry") }

    // Shadow
    static func shadow(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/shadow") }
    static func shadowDesired(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/shadow/desired") }

    // Commands
    static func commands(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/commands") }

    // Config
    static func config(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/config") }

    // Logs
    static func logs(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/logs") }

    // OTA
    static func deviceOTA(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/ota") }
    static func otaDeployments(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/ota-deployments") }

    // Firmware
    static func firmwareUpdates(_ server: String) -> String { endpoint(server, "firmware-updates") }

    // Alerts
    static func alerts(_ server: String) -> String { endpoint(server, "alerts") }
    static func alertAcknowledge(_ server: String, id: String) -> String { endpoint(server, "alerts/\(id)/acknowledge") }
    static func alertResolve(_ server: String, id: String) -> String { endpoint(server, "alerts/\(id)/resolve") }

    // Rules
    static func rules(_ server: String) -> String { endpoint(server, "rules") }
    static func rule(_ server: String, id: String) -> String { endpoint(server, "rules/\(id)") }
    static func ruleEnabled(_ server: String, id: String) -> String { endpoint(server, "rules/\(id)/enabled") }

    // Alert actions
    static func alertReactivate(_ server: String, id: String) -> String { endpoint(server, "alerts/\(id)/reactivate") }

    // Firmware detail
    static func firmwareUpdate(_ server: String, id: Int) -> String { endpoint(server, "firmware-updates/\(id)") }

    // Location
    static func deviceLocationLatest(_ server: String, deviceId: String) -> String { endpoint(server, "devices/\(deviceId)/location/latest") }

    // Zones
    static func zones(_ server: String) -> String { endpoint(server, "zones") }

    // Supporting
    static func deviceTypes(_ server: String) -> String { endpoint(server, "device-types") }
    static func deviceType(_ server: String, id: Int) -> String { endpoint(server, "device-types/\(id)") }
    static func fleets(_ server: String) -> String { endpoint(server, "fleets") }
    static func fleet(_ server: String, id: Int) -> String { endpoint(server, "fleets/\(id)") }

    private static func endpoint(
        _ server: String,
        _ path: String,
        queryItems: [URLQueryItem] = []
    ) -> String {
        APIConfiguration.current
            .url(serverAddress: server, endpointPath: path, queryItems: queryItems)?
            .absoluteString ?? ""
    }
}
