import Foundation

protocol OTARepository: Sendable {
    func getDeployments(deviceId: String) async throws -> [OtaDeployment]
    func getFirmwareForDeviceType(deviceTypeId: Int) async throws -> [FirmwareUpdate]
    func triggerOTA(deviceId: String, firmwareUpdateId: Int) async throws
}
