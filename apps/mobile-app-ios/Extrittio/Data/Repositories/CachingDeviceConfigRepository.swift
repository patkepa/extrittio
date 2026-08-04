import Foundation

final class CachingDeviceConfigRepository: DeviceConfigRepository, Sendable {
    private let remote: any DeviceConfigRepository
    private let cacheManager: CacheManager

    init(remote: any DeviceConfigRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getConfig(deviceId: String) async throws -> [String: AnyCodableValue] {
        do {
            let config = try await remote.getConfig(deviceId: deviceId)
            await cacheManager.cacheConfig(config, deviceId: deviceId)
            return config
        } catch {
            if let cached = await cacheManager.getConfig(deviceId: deviceId) { return cached }
            throw error
        }
    }

    func updateConfig(deviceId: String, config: [String: AnyCodableValue]) async throws -> [String: AnyCodableValue] {
        let updated = try await remote.updateConfig(deviceId: deviceId, config: config)
        await cacheManager.cacheConfig(updated, deviceId: deviceId)
        return updated
    }
}
