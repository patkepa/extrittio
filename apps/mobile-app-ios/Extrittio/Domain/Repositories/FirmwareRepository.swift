import Foundation

protocol FirmwareRepository: Sendable {
    func getFirmwareList(deviceTypeId: Int?, limit: Int, offset: Int) async throws -> PaginatedResponse<FirmwareUpdate>
    func deleteFirmware(id: Int) async throws
}
