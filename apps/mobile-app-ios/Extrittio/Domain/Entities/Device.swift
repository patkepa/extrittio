import Foundation

struct Device: Codable, Sendable, Identifiable, Hashable {
    let id: String
    let name: String
    let deviceTypeId: Int
    let deviceTypeName: String?
    let fleetId: Int?
    let fleetName: String?
    let status: String
    let firmware: String
    let lastSeen: String?
    let lastSeenAt: String?
    let uptime: String?
    let uptimeSeconds: Int
    let latestLatitude: Double?
    let latestLongitude: Double?

    enum CodingKeys: String, CodingKey {
        case id, name, status, firmware, uptime
        case deviceTypeId = "device_type_id"
        case deviceTypeName = "device_type_name"
        case fleetId = "fleet_id"
        case fleetName = "fleet_name"
        case lastSeen = "last_seen"
        case lastSeenAt = "last_seen_at"
        case uptimeSeconds = "uptime_seconds"
        case latestLatitude = "latest_latitude"
        case latestLongitude = "latest_longitude"
    }

    var hasLocation: Bool {
        latestLatitude != nil && latestLongitude != nil
    }

    var isOnline: Bool { status == "online" }
    var isOffline: Bool { status == "offline" }

    var firmwareVersionLabel: String {
        let trimmedFirmware = firmware.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedFirmware.isEmpty, trimmedFirmware.lowercased() != "unknown" else {
            return "Unknown"
        }
        return trimmedFirmware.lowercased().hasPrefix("v") ? trimmedFirmware : "v\(trimmedFirmware)"
    }

    var statusColor: String {
        switch status {
        case "online": "green"
        case "offline": "red"
        case "warning": "yellow"
        default: "gray"
        }
    }
}
