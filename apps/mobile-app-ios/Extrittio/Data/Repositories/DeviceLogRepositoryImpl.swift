import Foundation

final class DeviceLogRepositoryImpl: DeviceLogRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getLogs(deviceId: String, limit: Int, level: String?) async throws -> [DeviceLog] {
        let server = await apiClient.serverAddress
        var url = "\(Endpoints.logs(server, deviceId: deviceId))?limit=\(limit)"
        if let level { url += "&level=\(level)" }
        return try await apiClient.get(url)
    }
}
