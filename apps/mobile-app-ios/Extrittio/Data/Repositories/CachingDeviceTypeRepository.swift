import Foundation

final class CachingDeviceTypeRepository: DeviceTypeRepository, Sendable {
    private let remote: any DeviceTypeRepository
    private let cacheManager: CacheManager

    init(remote: any DeviceTypeRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getDeviceTypes() async throws -> [DeviceType] {
        do {
            let types = try await remote.getDeviceTypes()
            await cacheManager.cacheDeviceTypes(types)
            return types
        } catch {
            if let cached = await cacheManager.getDeviceTypes() { return cached }
            throw error
        }
    }

    func createDeviceType(_ request: CreateDeviceTypeRequest) async throws -> DeviceType {
        let type = try await remote.createDeviceType(request)
        await refreshCache()
        return type
    }

    func updateDeviceType(id: Int, _ request: UpdateDeviceTypeRequest) async throws -> DeviceType {
        let type = try await remote.updateDeviceType(id: id, request)
        await refreshCache()
        return type
    }

    func deleteDeviceType(id: Int) async throws {
        try await remote.deleteDeviceType(id: id)
        await refreshCache()
    }

    private func refreshCache() async {
        if let types = try? await remote.getDeviceTypes() {
            await cacheManager.cacheDeviceTypes(types)
        }
    }
}
