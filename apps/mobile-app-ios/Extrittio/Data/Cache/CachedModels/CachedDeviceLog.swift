import Foundation
import SwiftData

@Model
final class CachedDeviceLog {
    @Attribute(.unique) var id: Int
    var deviceId: String
    var level: String
    var message: String
    var createdAt: String
    var cachedAt: Date

    init(id: Int, deviceId: String, level: String, message: String,
         createdAt: String, cachedAt: Date = Date()) {
        self.id = id; self.deviceId = deviceId; self.level = level
        self.message = message; self.createdAt = createdAt; self.cachedAt = cachedAt
    }

    func toDomain() -> DeviceLog {
        DeviceLog(id: id, deviceId: deviceId, level: level, message: message, createdAt: createdAt)
    }

    static func from(_ log: DeviceLog) -> CachedDeviceLog {
        CachedDeviceLog(id: log.id, deviceId: log.deviceId, level: log.level,
                        message: log.message, createdAt: log.createdAt)
    }
}
