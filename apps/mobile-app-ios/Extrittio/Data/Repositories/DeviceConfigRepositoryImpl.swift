import Foundation

final class DeviceConfigRepositoryImpl: DeviceConfigRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getConfig(deviceId: String) async throws -> [String: AnyCodableValue] {
        let server = await apiClient.serverAddress
        do {
            let response: DeviceConfigResponse = try await apiClient.get(Endpoints.config(server, deviceId: deviceId))
            return response.config
        } catch let error as APIError where error == .notFound {
            return [:]
        }
    }

    func updateConfig(deviceId: String, config: [String: AnyCodableValue]) async throws -> [String: AnyCodableValue] {
        let server = await apiClient.serverAddress
        let response: DeviceConfigResponse = try await apiClient.put(Endpoints.config(server, deviceId: deviceId), body: config)
        return response.config
    }
}
