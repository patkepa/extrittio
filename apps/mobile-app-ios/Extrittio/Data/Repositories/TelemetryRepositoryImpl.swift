import Foundation

final class TelemetryRepositoryImpl: TelemetryRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getTelemetry(deviceId: String, limit: Int, since: String?) async throws -> [TelemetryRecord] {
        let server = await apiClient.serverAddress
        var url = "\(Endpoints.telemetry(server, deviceId: deviceId))?limit=\(limit)"
        if let since { url += "&since=\(since)" }
        return try await apiClient.get(url)
    }
}
