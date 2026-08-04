import Foundation

final class DashboardRepositoryImpl: DashboardRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getStats() async throws -> DashboardStats {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.dashboardStats(server))
    }
}
