import Foundation

final class ZoneRepositoryImpl: ZoneRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getZones() async throws -> [Zone] {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.zones(server))
    }
}
