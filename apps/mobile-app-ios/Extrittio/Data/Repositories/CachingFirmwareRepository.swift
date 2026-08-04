import Foundation

final class CachingFirmwareRepository: FirmwareRepository, Sendable {
    private let remote: any FirmwareRepository
    private let cacheManager: CacheManager

    init(remote: any FirmwareRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getFirmwareList(deviceTypeId: Int?, limit: Int, offset: Int) async throws -> PaginatedResponse<FirmwareUpdate> {
        do {
            let response = try await remote.getFirmwareList(deviceTypeId: deviceTypeId, limit: limit, offset: offset)
            await cacheManager.cacheFirmware(response.data)
            return response
        } catch {
            if let cached = await cacheManager.getFirmware() {
                return PaginatedResponse(data: cached, total: cached.count, limit: limit, offset: 0)
            }
            throw error
        }
    }

    func deleteFirmware(id: Int) async throws {
        try await remote.deleteFirmware(id: id)
        await cacheManager.removeFirmware(id: id)
    }
}
