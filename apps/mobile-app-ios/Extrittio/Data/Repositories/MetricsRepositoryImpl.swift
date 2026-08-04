import Foundation

final class MetricsRepositoryImpl: MetricsRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getCurrentMetrics() async throws -> CurrentMetricsResponse {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.serverMetricsCurrent(server))
    }

    func getMetricsHistory(since: String, resolution: Int) async throws -> MetricsHistoryResponse {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.serverMetricsHistory(server, since: since, resolution: resolution))
    }
}
