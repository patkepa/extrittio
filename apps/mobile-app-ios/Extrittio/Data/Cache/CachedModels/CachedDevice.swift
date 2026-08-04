import Foundation
import SwiftData

@Model
final class CachedDevice {
    @Attribute(.unique) var id: String
    var name: String
    var deviceTypeId: Int
    var deviceTypeName: String?
    var fleetId: Int?
    var fleetName: String?
    var status: String
    var firmware: String
    var lastSeen: String?
    var lastSeenAt: String?
    var uptime: String?
    var uptimeSeconds: Int
    var latestLatitude: Double?
    var latestLongitude: Double?
    var cachedAt: Date

    init(id: String, name: String, deviceTypeId: Int, deviceTypeName: String?,
         fleetId: Int?, fleetName: String?, status: String, firmware: String,
         lastSeen: String?, lastSeenAt: String?, uptime: String?, uptimeSeconds: Int,
         latestLatitude: Double?, latestLongitude: Double?, cachedAt: Date = Date()) {
        self.id = id; self.name = name; self.deviceTypeId = deviceTypeId
        self.deviceTypeName = deviceTypeName; self.fleetId = fleetId
        self.fleetName = fleetName; self.status = status; self.firmware = firmware
        self.lastSeen = lastSeen; self.lastSeenAt = lastSeenAt; self.uptime = uptime
        self.uptimeSeconds = uptimeSeconds; self.latestLatitude = latestLatitude
        self.latestLongitude = latestLongitude; self.cachedAt = cachedAt
    }

    func toDomain() -> Device {
        Device(id: id, name: name, deviceTypeId: deviceTypeId, deviceTypeName: deviceTypeName,
               fleetId: fleetId, fleetName: fleetName, status: status, firmware: firmware,
               lastSeen: lastSeen, lastSeenAt: lastSeenAt, uptime: uptime,
               uptimeSeconds: uptimeSeconds, latestLatitude: latestLatitude,
               latestLongitude: latestLongitude)
    }

    static func from(_ device: Device) -> CachedDevice {
        CachedDevice(id: device.id, name: device.name, deviceTypeId: device.deviceTypeId,
                     deviceTypeName: device.deviceTypeName, fleetId: device.fleetId,
                     fleetName: device.fleetName, status: device.status, firmware: device.firmware,
                     lastSeen: device.lastSeen, lastSeenAt: device.lastSeenAt, uptime: device.uptime,
                     uptimeSeconds: device.uptimeSeconds, latestLatitude: device.latestLatitude,
                     latestLongitude: device.latestLongitude)
    }
}
