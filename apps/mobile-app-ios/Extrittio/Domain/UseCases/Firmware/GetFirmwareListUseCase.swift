import Foundation

struct GetFirmwareListUseCase: Sendable {
    private let firmwareRepository: any FirmwareRepository
    private let deviceTypeRepository: any DeviceTypeRepository

    init(firmwareRepository: any FirmwareRepository, deviceTypeRepository: any DeviceTypeRepository) {
        self.firmwareRepository = firmwareRepository
        self.deviceTypeRepository = deviceTypeRepository
    }

    func execute(deviceTypeId: Int?, limit: Int = 100, offset: Int = 0) async throws -> (firmware: PaginatedResponse<FirmwareUpdate>, deviceTypes: [DeviceType]) {
        async let fw = firmwareRepository.getFirmwareList(deviceTypeId: deviceTypeId, limit: limit, offset: offset)
        async let types = deviceTypeRepository.getDeviceTypes()
        return (try await fw, try await types)
    }
}
