import Foundation

final class CachingDeviceLogRepository: DeviceLogRepository, Sendable {
    private let remote: any DeviceLogRepository
    private let cacheManager: CacheManager

    init(remote: any DeviceLogRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getLogs(deviceId: String, limit: Int, level: String?) async throws -> [DeviceLog] {
        do {
            let logs = try await remote.getLogs(deviceId: deviceId, limit: limit, level: level)
            await cacheManager.cacheLogs(logs, deviceId: deviceId)
            return logs
        } catch {
            if let cached = await cacheManager.getLogs(deviceId: deviceId) { return cached }
            throw error
        }
    }
}
