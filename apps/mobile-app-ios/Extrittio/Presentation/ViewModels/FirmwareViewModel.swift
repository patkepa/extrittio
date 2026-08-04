import Foundation
import os

@Observable
@MainActor
final class FirmwareViewModel {
    var firmwareState: ViewState<[FirmwareUpdate]> = .loading
    var deviceTypes: [DeviceType] = []
    var totalFirmware = 0
    var selectedDeviceTypeId: Int?

    private let getFirmwareListUseCase: GetFirmwareListUseCase
    private let deleteFirmwareUseCase: DeleteFirmwareUseCase
    private let cacheMetadata: any CacheMetadataProvider
    private let connectionMonitor: ConnectionMonitor
    private let logger = Logger(subsystem: "com.extrittio", category: "Firmware")

    init(getFirmwareListUseCase: GetFirmwareListUseCase, deleteFirmwareUseCase: DeleteFirmwareUseCase, cacheMetadata: any CacheMetadataProvider, connectionMonitor: ConnectionMonitor) {
        self.getFirmwareListUseCase = getFirmwareListUseCase
        self.deleteFirmwareUseCase = deleteFirmwareUseCase
        self.cacheMetadata = cacheMetadata
        self.connectionMonitor = connectionMonitor
    }

    func load() async {
        firmwareState = .loading
        do {
            let result = try await getFirmwareListUseCase.execute(deviceTypeId: selectedDeviceTypeId)
            let firmware = result.firmware.data
            totalFirmware = result.firmware.total
            deviceTypes = result.deviceTypes
            if firmware.isEmpty {
                firmwareState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .firmware) {
                firmwareState = .cached(firmware, lastUpdated: lastUpdated)
            } else {
                firmwareState = .loaded(firmware)
            }
        } catch {
            firmwareState = .error(error)
            logger.error("Firmware load failed: \(error)")
        }
    }

    @discardableResult
    func deleteFirmware(id: Int) async -> Bool {
        do {
            try await deleteFirmwareUseCase.execute(id: id)
            if var firmware = firmwareState.data {
                firmware.removeAll { $0.id == id }
                totalFirmware -= 1
                firmwareState = firmware.isEmpty ? .empty : .loaded(firmware)
            }
            return true
        } catch {
            firmwareState = .error(error)
            logger.error("Firmware delete failed: \(error)")
            return false
        }
    }
}
