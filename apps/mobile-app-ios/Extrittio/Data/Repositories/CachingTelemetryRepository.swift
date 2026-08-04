import Foundation

final class CachingTelemetryRepository: TelemetryRepository, Sendable {
    private let remote: any TelemetryRepository
    private let cacheManager: CacheManager

    init(remote: any TelemetryRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getTelemetry(deviceId: String, limit: Int, since: String?) async throws -> [TelemetryRecord] {
        do {
            let records = try await remote.getTelemetry(deviceId: deviceId, limit: limit, since: since)
            await cacheManager.cacheTelemetry(records, deviceId: deviceId)
            return records
        } catch {
            if let cached = await cacheManager.getTelemetry(deviceId: deviceId) { return cached }
            throw error
        }
    }
}
