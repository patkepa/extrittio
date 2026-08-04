import Foundation
import SwiftData

@Model
final class CachedDeviceConfig {
    @Attribute(.unique) var deviceId: String
    var configData: Data
    var cachedAt: Date

    init(deviceId: String, configData: Data, cachedAt: Date = Date()) {
        self.deviceId = deviceId; self.configData = configData; self.cachedAt = cachedAt
    }

    func toDomain() -> [String: AnyCodableValue] {
        let decoder = JSONDecoder()
        return (try? decoder.decode([String: AnyCodableValue].self, from: configData)) ?? [:]
    }

    static func from(_ config: [String: AnyCodableValue], deviceId: String) -> CachedDeviceConfig {
        let encoder = JSONEncoder()
        let data = (try? encoder.encode(config)) ?? Data()
        return CachedDeviceConfig(deviceId: deviceId, configData: data)
    }
}
