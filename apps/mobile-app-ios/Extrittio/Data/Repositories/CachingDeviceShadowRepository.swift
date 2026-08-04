import Foundation

final class CachingDeviceShadowRepository: DeviceShadowRepository, Sendable {
    private let remote: any DeviceShadowRepository
    private let cacheManager: CacheManager

    init(remote: any DeviceShadowRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getShadow(deviceId: String) async throws -> DeviceShadow? {
        do {
            let shadow = try await remote.getShadow(deviceId: deviceId)
            if let shadow { await cacheManager.cacheShadow(shadow) }
            return shadow
        } catch {
            return await cacheManager.getShadow(deviceId: deviceId)
        }
    }

    func updateDesired(deviceId: String, desired: [String: AnyCodableValue]) async throws -> DeviceShadow {
        let shadow = try await remote.updateDesired(deviceId: deviceId, desired: desired)
        await cacheManager.cacheShadow(shadow)
        return shadow
    }

    func deleteShadow(deviceId: String) async throws {
        try await remote.deleteShadow(deviceId: deviceId)
        await cacheManager.removeShadow(deviceId: deviceId)
    }
}
