import Foundation

struct DeviceLocation: Codable, Sendable {
    let latitude: Double
    let longitude: Double
    let speed: Double?
    let altitude: Double?
    let heading: Double?
    let timestamp: String
}
