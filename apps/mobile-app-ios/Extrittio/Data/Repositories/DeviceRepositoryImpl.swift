import Foundation

final class DeviceRepositoryImpl: DeviceRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getDevices(status: String?, search: String?, fleetId: Int?, limit: Int, offset: Int) async throws -> PaginatedResponse<Device> {
        let server = await apiClient.serverAddress
        var url = "\(Endpoints.devices(server))?limit=\(limit)&offset=\(offset)"
        if let status, !status.isEmpty { url += "&status=\(status)" }
        if let search, !search.isEmpty {
            url += "&search=\(search.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? search)"
        }
        if let fleetId { url += "&fleet_id=\(fleetId)" }
        return try await apiClient.get(url)
    }

    func getDevice(id: String) async throws -> Device {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.device(server, id: id))
    }

    func createDevice(_ request: CreateDeviceRequest) async throws -> Device {
        let server = await apiClient.serverAddress
        return try await apiClient.post(Endpoints.devices(server), body: request)
    }

    func updateDevice(id: String, _ request: UpdateDeviceRequest) async throws -> Device {
        let server = await apiClient.serverAddress
        return try await apiClient.put(Endpoints.device(server, id: id), body: request)
    }

    func deleteDevice(id: String) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.device(server, id: id))
    }

    func restartDevice(id: String) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.postNoContent(Endpoints.deviceRestart(server, id: id))
    }

    func getLatestLocation(deviceId: String) async throws -> DeviceLocation? {
        let server = await apiClient.serverAddress
        do {
            let location: DeviceLocation = try await apiClient.get(Endpoints.deviceLocationLatest(server, deviceId: deviceId))
            return location
        } catch APIError.notFound {
            return nil
        } catch APIError.decodingError {
            return nil
        }
    }
}
