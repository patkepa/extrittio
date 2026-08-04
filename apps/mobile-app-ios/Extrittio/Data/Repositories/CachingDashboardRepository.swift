import Foundation

final class CachingDashboardRepository: DashboardRepository, Sendable {
    private let remote: any DashboardRepository
    private let cacheManager: CacheManager

    init(remote: any DashboardRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getStats() async throws -> DashboardStats {
        do {
            let stats = try await remote.getStats()
            await cacheManager.cacheDashboardStats(stats)
            return stats
        } catch {
            if let cached = await cacheManager.getDashboardStats() { return cached }
            throw error
        }
    }
}
