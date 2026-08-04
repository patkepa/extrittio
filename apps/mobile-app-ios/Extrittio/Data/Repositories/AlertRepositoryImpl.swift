import Foundation

final class AlertRepositoryImpl: AlertRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getAlerts(deviceId: String?, status: String?, severity: String?, limit: Int, offset: Int) async throws -> PaginatedResponse<Alert> {
        let server = await apiClient.serverAddress
        var url = "\(Endpoints.alerts(server))?limit=\(limit)&offset=\(offset)"
        if let deviceId { url += "&device_id=\(deviceId)" }
        if let status { url += "&status=\(status)" }
        if let severity { url += "&severity=\(severity)" }
        return try await apiClient.get(url)
    }

    func getAlertSummary() async throws -> AlertSummary {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.alertSummary(server))
    }

    func acknowledgeAlert(id: String) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.putNoContent(Endpoints.alertAcknowledge(server, id: id))
    }

    func resolveAlert(id: String) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.putNoContent(Endpoints.alertResolve(server, id: id))
    }

    func reactivateAlert(id: String) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.putNoContent(Endpoints.alertReactivate(server, id: id))
    }
}
