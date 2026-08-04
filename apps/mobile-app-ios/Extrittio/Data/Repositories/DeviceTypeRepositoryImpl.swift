import Foundation

final class DeviceTypeRepositoryImpl: DeviceTypeRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getDeviceTypes() async throws -> [DeviceType] {
        let server = await apiClient.serverAddress
        let response: PaginatedResponse<DeviceType> = try await apiClient.get(Endpoints.deviceTypes(server))
        return response.data
    }

    func createDeviceType(_ request: CreateDeviceTypeRequest) async throws -> DeviceType {
        let server = await apiClient.serverAddress
        return try await apiClient.post(Endpoints.deviceTypes(server), body: request)
    }

    func updateDeviceType(id: Int, _ request: UpdateDeviceTypeRequest) async throws -> DeviceType {
        let server = await apiClient.serverAddress
        return try await apiClient.patch(Endpoints.deviceType(server, id: id), body: request)
    }

    func deleteDeviceType(id: Int) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.deviceType(server, id: id))
    }
}
