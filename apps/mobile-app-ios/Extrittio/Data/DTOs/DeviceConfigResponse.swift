import Foundation

struct DeviceConfigResponse: Codable, Sendable {
    let deviceId: String
    let config: [String: AnyCodableValue]
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case config
        case deviceId = "device_id"
        case updatedAt = "updated_at"
    }
}
