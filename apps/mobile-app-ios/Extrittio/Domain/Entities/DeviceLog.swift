import Foundation

struct DeviceLog: Codable, Sendable, Identifiable {
    let id: Int
    let deviceId: String
    let level: String
    let message: String
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id, level, message
        case deviceId = "device_id"
        case createdAt = "created_at"
    }

    var levelColor: String {
        switch level {
        case "ERROR": "red"
        case "WARN": "yellow"
        case "INFO": "blue"
        case "DEBUG": "gray"
        default: "gray"
        }
    }
}
