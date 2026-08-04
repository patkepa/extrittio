import Foundation

final class CachingZoneRepository: ZoneRepository, Sendable {
    private let remote: any ZoneRepository
    private let cacheManager: CacheManager

    init(remote: any ZoneRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getZones() async throws -> [Zone] {
        do {
            let zones = try await remote.getZones()
            await cacheManager.cacheZones(zones)
            return zones
        } catch {
            if let cached = await cacheManager.getZones() { return cached }
            throw error
        }
    }
}
