import Foundation

final class CachingOTARepository: OTARepository, Sendable {
    private let remote: any OTARepository
    private let cacheManager: CacheManager

    init(remote: any OTARepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getDeployments(deviceId: String) async throws -> [OtaDeployment] {
        do {
            let deployments = try await remote.getDeployments(deviceId: deviceId)
            await cacheManager.cacheOtaDeployments(deployments, deviceId: deviceId)
            return deployments
        } catch {
            if let cached = await cacheManager.getOtaDeployments(deviceId: deviceId) { return cached }
            throw error
        }
    }

    func getFirmwareForDeviceType(deviceTypeId: Int) async throws -> [FirmwareUpdate] {
        do {
            let firmware = try await remote.getFirmwareForDeviceType(deviceTypeId: deviceTypeId)
            await cacheManager.cacheFirmware(firmware)
            return firmware
        } catch {
            if let cached = await cacheManager.getFirmware() { return cached }
            throw error
        }
    }

    func triggerOTA(deviceId: String, firmwareUpdateId: Int) async throws {
        try await remote.triggerOTA(deviceId: deviceId, firmwareUpdateId: firmwareUpdateId)
    }
}
