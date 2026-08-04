import Foundation

final class FirmwareRepositoryImpl: FirmwareRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getFirmwareList(deviceTypeId: Int?, limit: Int, offset: Int) async throws -> PaginatedResponse<FirmwareUpdate> {
        let server = await apiClient.serverAddress
        var url = "\(Endpoints.firmwareUpdates(server))?limit=\(limit)&offset=\(offset)"
        if let typeId = deviceTypeId { url += "&device_type_id=\(typeId)" }
        return try await apiClient.get(url)
    }

    func deleteFirmware(id: Int) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.firmwareUpdate(server, id: id))
    }
}
