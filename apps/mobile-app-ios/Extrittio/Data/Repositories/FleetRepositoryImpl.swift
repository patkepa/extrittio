import Foundation

final class FleetRepositoryImpl: FleetRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getFleets() async throws -> [Fleet] {
        let server = await apiClient.serverAddress
        let response: PaginatedResponse<Fleet> = try await apiClient.get(Endpoints.fleets(server))
        return response.data
    }

    func createFleet(_ request: CreateFleetRequest) async throws -> Fleet {
        let server = await apiClient.serverAddress
        return try await apiClient.post(Endpoints.fleets(server), body: request)
    }

    func updateFleet(id: Int, _ request: UpdateFleetRequest) async throws -> Fleet {
        let server = await apiClient.serverAddress
        return try await apiClient.patch(Endpoints.fleet(server, id: id), body: request)
    }

    func deleteFleet(id: Int) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.fleet(server, id: id))
    }
}
