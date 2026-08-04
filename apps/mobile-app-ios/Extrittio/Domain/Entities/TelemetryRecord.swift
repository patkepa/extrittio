import Foundation

struct TelemetryRecord: Codable, Sendable, Identifiable {
    let id: Int
    let deviceId: String
    let temperature: Double?
    let humidity: Double?
    let batteryLevel: Double?
    let customJson: String?
    let latitude: Double?
    let longitude: Double?
    let speed: Double?
    let altitude: Double?
    let heading: Double?
    let receivedAt: String

    enum CodingKeys: String, CodingKey {
        case id, temperature, humidity, latitude, longitude, speed, altitude, heading
        case deviceId = "device_id"
        case batteryLevel = "battery_level"
        case customJson = "custom_json"
        case receivedAt = "received_at"
    }

    var hasLocation: Bool {
        latitude != nil && longitude != nil
    }
}
