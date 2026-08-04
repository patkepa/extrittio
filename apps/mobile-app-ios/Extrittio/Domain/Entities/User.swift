import Foundation

struct User: Codable, Sendable, Identifiable {
    let id: Int
    let username: String
    let role: String
    let roles: [RoleSummary]?
    let permissions: [String]?
    let permissionVersion: Int?

    enum CodingKeys: String, CodingKey {
        case id, username, role, roles, permissions
        case permissionVersion = "permission_version"
    }

    func hasPermission(_ permission: PermissionKey) -> Bool {
        if permissions == nil {
            return role == "admin" || role == "owner"
        }
        return PermissionKey.hasPermission(permissions, permission)
    }

    func hasRequiredPermissions(_ required: [PermissionKey]) -> Bool {
        required.allSatisfy { hasPermission($0) }
    }
}

struct RoleSummary: Codable, Sendable, Identifiable, Hashable {
    let id: Int
    let name: String
    let description: String?
    let isSystem: Bool

    enum CodingKeys: String, CodingKey {
        case id, name, description
        case isSystem = "is_system"
    }
}

enum PermissionKey: String, CaseIterable, Sendable, Identifiable {
    case apiKeysManage = "api_keys.manage"
    case alertsManage = "alerts.manage"
    case alertsRead = "alerts.read"
    case commandsRead = "commands.read"
    case commandsSend = "commands.send"
    case deviceTypesManage = "device_types.manage"
    case deviceTypesRead = "device_types.read"
    case devicesManage = "devices.manage"
    case devicesRead = "devices.read"
    case firmwareDeploy = "firmware.deploy"
    case firmwareManage = "firmware.manage"
    case firmwareRead = "firmware.read"
    case fleetsManage = "fleets.manage"
    case fleetsRead = "fleets.read"
    case logsRead = "logs.read"
    case rolesManage = "roles.manage"
    case rolesRead = "roles.read"
    case rulesManage = "rules.manage"
    case rulesRead = "rules.read"
    case serverMetricsRead = "server_metrics.read"
    case shadowsManage = "shadows.manage"
    case shadowsRead = "shadows.read"
    case telemetryRead = "telemetry.read"
    case usersManage = "users.manage"
    case usersRead = "users.read"
    case zonesManage = "zones.manage"
    case zonesRead = "zones.read"

    var id: String { rawValue }

    var label: String {
        rawValue
            .replacingOccurrences(of: "_", with: " ")
            .replacingOccurrences(of: ".", with: " ")
            .capitalized
    }

    static func hasPermission(_ permissions: [String]?, _ permission: PermissionKey) -> Bool {
        guard let permissions else { return false }
        if permissions.contains(permission.rawValue) { return true }
        return impliedPermissions[permission, default: []].contains { permissions.contains($0.rawValue) }
    }

    private static let impliedPermissions: [PermissionKey: [PermissionKey]] = [
        .alertsRead: [.alertsManage],
        .commandsRead: [.commandsSend],
        .deviceTypesRead: [.deviceTypesManage],
        .devicesRead: [.devicesManage],
        .firmwareRead: [.firmwareManage, .firmwareDeploy],
        .fleetsRead: [.fleetsManage],
        .rolesRead: [.rolesManage],
        .rulesRead: [.rulesManage],
        .shadowsRead: [.shadowsManage],
        .usersRead: [.usersManage],
        .zonesRead: [.zonesManage]
    ]
}
