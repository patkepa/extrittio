import Foundation

final class DeviceBlueprintRepositoryImpl: DeviceBlueprintRepository, Sendable {
    private let apiClient: APIClient

    init(apiClient: APIClient) {
        self.apiClient = apiClient
    }

    func getBlueprints() async throws -> [DeviceBlueprint] {
        let server = await apiClient.serverAddress
        let response: PaginatedResponse<DeviceBlueprint> = try await apiClient.get(
            "\(Endpoints.deviceBlueprints(server))?limit=500&offset=0"
        )
        return response.data
    }

    func getLatestRevision(blueprintId: String) async throws -> DeviceBlueprintRevision {
        let server = await apiClient.serverAddress
        return try await apiClient.get(
            Endpoints.latestDeviceBlueprintRevision(server, id: blueprintId)
        )
    }
}
