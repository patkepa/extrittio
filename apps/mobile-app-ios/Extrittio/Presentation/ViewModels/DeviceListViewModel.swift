import Foundation
import os

struct DeviceBulkActionResult: Equatable, Sendable {
    let requestedIds: [String]
    let succeededIds: Set<String>
    let failedIds: Set<String>

    var requestedCount: Int { requestedIds.count }
    var succeededCount: Int { succeededIds.count }
    var failedCount: Int { failedIds.count }
    var didSucceedCompletely: Bool { failedIds.isEmpty && !requestedIds.isEmpty }
}

@Observable
@MainActor
final class DeviceListViewModel {
    var devicesState: ViewState<[Device]> = .loading
    var totalDevices = 0
    var searchText = ""
    var statusFilter: String?
    var fleets: [Fleet] = []
    var selectedFleetId: Int?

    private let getDevicesUseCase: GetDevicesUseCase
    private let getFleetsUseCase: GetFleetsUseCase
    private let deleteDeviceUseCase: DeleteDeviceUseCase
    private let restartDeviceUseCase: RestartDeviceUseCase
    private let cacheMetadata: any CacheMetadataProvider
    private let connectionMonitor: ConnectionMonitor
    private let logger = Logger(subsystem: "com.extrittio", category: "DeviceList")
    private let pageSize = 30
    private var currentOffset = 0
    private var hasMore = true
    private var searchTask: Task<Void, Never>?

    init(getDevicesUseCase: GetDevicesUseCase, getFleetsUseCase: GetFleetsUseCase, deleteDeviceUseCase: DeleteDeviceUseCase, restartDeviceUseCase: RestartDeviceUseCase, cacheMetadata: any CacheMetadataProvider, connectionMonitor: ConnectionMonitor) {
        self.getDevicesUseCase = getDevicesUseCase
        self.getFleetsUseCase = getFleetsUseCase
        self.deleteDeviceUseCase = deleteDeviceUseCase
        self.restartDeviceUseCase = restartDeviceUseCase
        self.cacheMetadata = cacheMetadata
        self.connectionMonitor = connectionMonitor
    }

    func load() async {
        if devicesState.data == nil {
            devicesState = .loading
        }
        currentOffset = 0
        hasMore = true

        do {
            async let devicesResponse = getDevicesUseCase.execute(
                status: statusFilter, search: searchText.isEmpty ? nil : searchText,
                fleetId: selectedFleetId, limit: pageSize, offset: 0
            )
            async let fleetsResponse = getFleetsUseCase.execute()
            let devResult = try await devicesResponse
            let devices = devResult.data
            totalDevices = devResult.total
            currentOffset = devices.count
            hasMore = currentOffset < devResult.total
            fleets = try await fleetsResponse
            if devices.isEmpty {
                devicesState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .devices) {
                devicesState = .cached(devices, lastUpdated: lastUpdated)
            } else {
                devicesState = .loaded(devices)
            }
        } catch {
            devicesState = .error(error)
            logger.error("Device list load failed: \(error)")
        }
    }

    func loadMore() async {
        guard hasMore, !devicesState.isLoading, !devicesState.isLoadingMore else { return }
        let existing = devicesState.data ?? []
        devicesState = .loadingMore(existing)
        do {
            let response = try await getDevicesUseCase.execute(
                status: statusFilter, search: searchText.isEmpty ? nil : searchText,
                fleetId: selectedFleetId, limit: pageSize, offset: currentOffset
            )
            let combined = existing + response.data
            currentOffset += response.data.count
            hasMore = currentOffset < response.total
            devicesState = combined.isEmpty ? .empty : .loaded(combined)
        } catch {
            // Restore the existing data on failure rather than losing it
            devicesState = existing.isEmpty ? .empty : .loaded(existing)
            logger.error("Load more failed: \(error)")
        }
    }

    func refresh() async {
        await load()
    }

    @discardableResult
    func deleteDevice(id: String) async -> Bool {
        do {
            try await deleteDeviceUseCase.execute(id: id)
            removeDevices(ids: [id])
            return true
        } catch {
            logger.error("Delete device failed: \(error)")
            return false
        }
    }

    @discardableResult
    func restartDevice(id: String) async -> Bool {
        do {
            try await restartDeviceUseCase.execute(id: id)
            return true
        } catch {
            logger.error("Restart device failed: \(error)")
            return false
        }
    }

    @discardableResult
    func deleteDevices(ids: Set<String>) async -> DeviceBulkActionResult {
        let orderedIds = orderedDeviceIds(in: ids)
        var succeededIds = Set<String>()
        var failedIds = Set<String>()

        for id in orderedIds {
            do {
                try await deleteDeviceUseCase.execute(id: id)
                succeededIds.insert(id)
            } catch {
                failedIds.insert(id)
                logger.error("Bulk delete failed for \(id, privacy: .public): \(error)")
            }
        }

        removeDevices(ids: succeededIds)
        return DeviceBulkActionResult(requestedIds: orderedIds, succeededIds: succeededIds, failedIds: failedIds)
    }

    @discardableResult
    func restartDevices(ids: Set<String>) async -> DeviceBulkActionResult {
        let orderedIds = orderedDeviceIds(in: ids)
        var succeededIds = Set<String>()
        var failedIds = Set<String>()

        for id in orderedIds {
            do {
                try await restartDeviceUseCase.execute(id: id)
                succeededIds.insert(id)
            } catch {
                failedIds.insert(id)
                logger.error("Bulk restart failed for \(id, privacy: .public): \(error)")
            }
        }

        return DeviceBulkActionResult(requestedIds: orderedIds, succeededIds: succeededIds, failedIds: failedIds)
    }

    func insertDevice(_ device: Device) {
        var devices = devicesState.data ?? []
        devices.insert(device, at: 0)
        totalDevices += 1
        devicesState = .loaded(devices)
    }

    func debouncedSearch() {
        searchTask?.cancel()
        searchTask = Task {
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            await load()
        }
    }

    private func orderedDeviceIds(in ids: Set<String>) -> [String] {
        let visibleIds = devicesState.data?.map(\.id).filter { ids.contains($0) } ?? []
        let visibleIdSet = Set(visibleIds)
        return visibleIds + ids.subtracting(visibleIdSet).sorted()
    }

    private func removeDevices(ids: Set<String>) {
        guard !ids.isEmpty, var devices = devicesState.data else { return }
        devices.removeAll { ids.contains($0.id) }
        totalDevices = max(totalDevices - ids.count, 0)
        devicesState = devices.isEmpty ? .empty : .loaded(devices)
    }
}
