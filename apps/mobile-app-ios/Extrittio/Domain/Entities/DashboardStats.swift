import Foundation

struct DashboardStats: Codable, Sendable {
    let totalDevices: Int
    let activeDevices: Int
    let offlineDevices: Int
    let totalMessages: Int

    enum CodingKeys: String, CodingKey {
        case totalDevices = "total_devices"
        case activeDevices = "active_devices"
        case offlineDevices = "offline_devices"
        case totalMessages = "total_messages"
    }
}

struct AlertSummary: Codable, Sendable {
    let active: AlertSeverityCounts
    let acknowledged: AlertSeverityCounts
    let totalActive: Int

    enum CodingKeys: String, CodingKey {
        case active, acknowledged
        case totalActive = "total_active"
    }
}

struct AlertSeverityCounts: Codable, Sendable {
    let info: Int
    let warning: Int
    let critical: Int
}
