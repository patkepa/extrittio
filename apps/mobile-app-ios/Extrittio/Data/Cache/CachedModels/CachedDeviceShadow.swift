import Foundation
import SwiftData

@Model
final class CachedDeviceShadow {
    @Attribute(.unique) var deviceId: String
    var desiredData: Data
    var reportedData: Data
    var deltaData: Data
    var version: Int
    var updatedAt: String
    var cachedAt: Date

    init(deviceId: String, desiredData: Data, reportedData: Data, deltaData: Data,
         version: Int, updatedAt: String, cachedAt: Date = Date()) {
        self.deviceId = deviceId; self.desiredData = desiredData
        self.reportedData = reportedData; self.deltaData = deltaData
        self.version = version; self.updatedAt = updatedAt; self.cachedAt = cachedAt
    }

    func toDomain() -> DeviceShadow {
        let decoder = JSONDecoder()
        let desired = (try? decoder.decode([String: AnyCodableValue].self, from: desiredData)) ?? [:]
        let reported = (try? decoder.decode([String: AnyCodableValue].self, from: reportedData)) ?? [:]
        let delta = (try? decoder.decode([String: AnyCodableValue].self, from: deltaData)) ?? [:]
        return DeviceShadow(deviceId: deviceId, desired: desired, reported: reported,
                            delta: delta, version: version, updatedAt: updatedAt)
    }

    static func from(_ shadow: DeviceShadow) -> CachedDeviceShadow {
        let encoder = JSONEncoder()
        let desired = (try? encoder.encode(shadow.desired)) ?? Data()
        let reported = (try? encoder.encode(shadow.reported)) ?? Data()
        let delta = (try? encoder.encode(shadow.delta)) ?? Data()
        return CachedDeviceShadow(deviceId: shadow.deviceId, desiredData: desired,
                                  reportedData: reported, deltaData: delta,
                                  version: shadow.version, updatedAt: shadow.updatedAt)
    }
}
