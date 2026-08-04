import Foundation

struct GetOTADataUseCase: Sendable {
    private let repository: any OTARepository
    init(repository: any OTARepository) { self.repository = repository }

    func execute(deviceId: String, deviceTypeId: Int?) async throws -> (deployments: [OtaDeployment], firmware: [FirmwareUpdate]) {
        let deployments = try await repository.getDeployments(deviceId: deviceId)
        var firmware: [FirmwareUpdate] = []
        if let typeId = deviceTypeId {
            firmware = try await repository.getFirmwareForDeviceType(deviceTypeId: typeId)
        }
        return (deployments, firmware)
    }
}
