import Foundation

final class OTARepositoryImpl: OTARepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getDeployments(deviceId: String) async throws -> [OtaDeployment] {
        let server = await apiClient.serverAddress
        let response: PaginatedResponse<OtaDeployment> = try await apiClient.get(Endpoints.otaDeployments(server, deviceId: deviceId))
        return response.data
    }

    func getFirmwareForDeviceType(deviceTypeId: Int) async throws -> [FirmwareUpdate] {
        let server = await apiClient.serverAddress
        let response: PaginatedResponse<FirmwareUpdate> = try await apiClient.get("\(Endpoints.firmwareUpdates(server))?device_type_id=\(deviceTypeId)")
        return response.data
    }

    func triggerOTA(deviceId: String, firmwareUpdateId: Int) async throws {
        let server = await apiClient.serverAddress
        let request = TriggerOTARequest(firmwareUpdateId: firmwareUpdateId)
        try await apiClient.postNoContent(Endpoints.deviceOTA(server, deviceId: deviceId), body: request)
    }
}
