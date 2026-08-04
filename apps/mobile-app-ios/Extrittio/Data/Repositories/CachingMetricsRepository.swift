import Foundation

final class CachingMetricsRepository: MetricsRepository, Sendable {
    private let remote: any MetricsRepository
    private let cacheManager: CacheManager

    init(remote: any MetricsRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getCurrentMetrics() async throws -> CurrentMetricsResponse {
        do {
            let metrics = try await remote.getCurrentMetrics()
            await cacheManager.cacheCurrentMetrics(metrics)
            return metrics
        } catch {
            if let cached = await cacheManager.getCurrentMetrics() { return cached }
            throw error
        }
    }

    func getMetricsHistory(since: String, resolution: Int) async throws -> MetricsHistoryResponse {
        do {
            let history = try await remote.getMetricsHistory(since: since, resolution: resolution)
            await cacheManager.cacheMetricsHistory(history)
            return history
        } catch {
            if let cached = await cacheManager.getMetricsHistory() { return cached }
            throw error
        }
    }
}
