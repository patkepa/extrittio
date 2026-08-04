import Foundation

protocol MetricsRepository: Sendable {
    func getCurrentMetrics() async throws -> CurrentMetricsResponse
    func getMetricsHistory(since: String, resolution: Int) async throws -> MetricsHistoryResponse
}
