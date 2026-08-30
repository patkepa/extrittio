import Foundation

final class CachingDeviceRepository: DeviceRepository, Sendable {
    private let remote: any DeviceRepository
    private let cacheManager: CacheManager

    init(remote: any DeviceRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getDevices(status: String?, search: String?, fleetId: Int?, limit: Int, offset: Int) async throws -> PaginatedResponse<Device> {
        do {
            let response = try await remote.getDevices(status: status, search: search, fleetId: fleetId, limit: limit, offset: offset)
            await cacheManager.cacheDevices(response.data)
            return response
        } catch {
            if let cached = await cacheManager.getDevices(status: status, search: search, fleetId: fleetId) {
                return PaginatedResponse(data: cached, total: cached.count, limit: limit, offset: 0)
            }
            throw error
        }
    }

    func getDevice(id: String) async throws -> Device {
        do {
            let device = try await remote.getDevice(id: id)
            await cacheManager.cacheDevice(device)
            return device
        } catch {
            if let cached = await cacheManager.getDevice(id: id) { return cached }
            throw error
        }
    }

    func getDeviceContract(id: String) async throws -> DeviceContract {
        try await remote.getDeviceContract(id: id)
    }

    func createDevice(_ request: CreateDeviceRequest) async throws -> Device {
        let device = try await remote.createDevice(request)
        await cacheManager.cacheDevice(device)
        return device
    }

    func updateDevice(id: String, _ request: UpdateDeviceRequest) async throws -> Device {
        let device = try await remote.updateDevice(id: id, request)
        await cacheManager.cacheDevice(device)
        return device
    }

    func deleteDevice(id: String) async throws {
        try await remote.deleteDevice(id: id)
        await cacheManager.removeDevice(id: id)
    }

    func restartDevice(id: String) async throws {
        try await remote.restartDevice(id: id)
    }

    func getLatestLocation(deviceId: String) async throws -> DeviceLocation? {
        do {
            let location = try await remote.getLatestLocation(deviceId: deviceId)
            if let location { await cacheManager.cacheLocation(location, deviceId: deviceId) }
            return location
        } catch {
            return await cacheManager.getLocation(deviceId: deviceId)
        }
    }
}
