import Foundation

final class CommandRepositoryImpl: CommandRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getCommands(deviceId: String, limit: Int, status: String?) async throws -> [CommandRecord] {
        let server = await apiClient.serverAddress
        var url = "\(Endpoints.commands(server, deviceId: deviceId))?limit=\(limit)"
        if let status { url += "&status=\(status)" }
        return try await apiClient.get(url)
    }

    func sendCommand(deviceId: String, command: String, params: [String: AnyCodableValue]?) async throws -> CommandRecord {
        let server = await apiClient.serverAddress
        let request = SendCommandRequest(command: command, params: params)
        return try await apiClient.post(Endpoints.commands(server, deviceId: deviceId), body: request)
    }
}
