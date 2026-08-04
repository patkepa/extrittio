import Foundation

final class CachingAlertRepository: AlertRepository, Sendable {
    private let remote: any AlertRepository
    private let cacheManager: CacheManager

    init(remote: any AlertRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getAlerts(deviceId: String?, status: String?, severity: String?, limit: Int, offset: Int) async throws -> PaginatedResponse<Alert> {
        do {
            let response = try await remote.getAlerts(deviceId: deviceId, status: status, severity: severity, limit: limit, offset: offset)
            if let deviceId {
                await cacheManager.cacheAlerts(response.data, deviceId: deviceId)
            }
            return response
        } catch {
            if let deviceId, let cached = await cacheManager.getAlerts(deviceId: deviceId) {
                return PaginatedResponse(data: cached, total: cached.count, limit: limit, offset: 0)
            }
            throw error
        }
    }

    func getAlertSummary() async throws -> AlertSummary {
        do {
            let summary = try await remote.getAlertSummary()
            await cacheManager.cacheAlertSummary(summary)
            return summary
        } catch {
            if let cached = await cacheManager.getAlertSummary() { return cached }
            throw error
        }
    }

    func acknowledgeAlert(id: String) async throws {
        try await remote.acknowledgeAlert(id: id)
    }

    func resolveAlert(id: String) async throws {
        try await remote.resolveAlert(id: id)
    }

    func reactivateAlert(id: String) async throws {
        try await remote.reactivateAlert(id: id)
    }
}
