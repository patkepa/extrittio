import Foundation

final class DeviceShadowRepositoryImpl: DeviceShadowRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getShadow(deviceId: String) async throws -> DeviceShadow? {
        let server = await apiClient.serverAddress
        do {
            return try await apiClient.get(Endpoints.shadow(server, deviceId: deviceId)) as DeviceShadow
        } catch let error as APIError where error == .notFound {
            return nil
        }
    }

    func updateDesired(deviceId: String, desired: [String: AnyCodableValue]) async throws -> DeviceShadow {
        let server = await apiClient.serverAddress
        return try await apiClient.put(Endpoints.shadowDesired(server, deviceId: deviceId), body: desired)
    }

    func deleteShadow(deviceId: String) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.shadow(server, deviceId: deviceId))
    }
}
