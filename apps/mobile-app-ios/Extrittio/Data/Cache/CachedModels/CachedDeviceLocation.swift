import Foundation
import SwiftData

@Model
final class CachedDeviceLocation {
    @Attribute(.unique) var deviceId: String
    var latitude: Double
    var longitude: Double
    var speed: Double?
    var altitude: Double?
    var heading: Double?
    var timestamp: String
    var cachedAt: Date

    init(deviceId: String, latitude: Double, longitude: Double, speed: Double?,
         altitude: Double?, heading: Double?, timestamp: String, cachedAt: Date = Date()) {
        self.deviceId = deviceId; self.latitude = latitude; self.longitude = longitude
        self.speed = speed; self.altitude = altitude; self.heading = heading
        self.timestamp = timestamp; self.cachedAt = cachedAt
    }

    func toDomain() -> DeviceLocation {
        DeviceLocation(latitude: latitude, longitude: longitude, speed: speed,
                       altitude: altitude, heading: heading, timestamp: timestamp)
    }

    static func from(_ loc: DeviceLocation, deviceId: String) -> CachedDeviceLocation {
        CachedDeviceLocation(deviceId: deviceId, latitude: loc.latitude, longitude: loc.longitude,
                             speed: loc.speed, altitude: loc.altitude, heading: loc.heading,
                             timestamp: loc.timestamp)
    }
}
