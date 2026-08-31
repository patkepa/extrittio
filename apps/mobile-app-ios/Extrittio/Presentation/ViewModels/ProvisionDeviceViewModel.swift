import Foundation
import Observation
import os

@Observable
@MainActor
final class ProvisionDeviceViewModel {
    enum Step: Equatable {
        case loading
        case configure
        case scan
        case registering
        case transfer
        case completed
    }

    var step: Step = .loading
    var name = ""
    var selectedBlueprintId = ""
    var selectedFleetId: Int?
    var configurationText = ""
    private(set) var blueprints: [DeviceBlueprint] = []
    private(set) var fleets: [Fleet] = []
    private(set) var revision: DeviceBlueprintRevision?
    private(set) var prepared: PreparedDeviceProvisioning?
    private(set) var completedDevice: Device?
    private(set) var errorMessage: String?
    private(set) var isRollingBack = false

    private let blueprintRepository: any DeviceBlueprintRepository
    private let fleetRepository: any FleetRepository
    private let provisionDevice: ProvisionDeviceUseCase
    private let logger = Logger(subsystem: "com.extrittio", category: "DeviceProvisioning")

    init(
        blueprintRepository: any DeviceBlueprintRepository,
        fleetRepository: any FleetRepository,
        provisionDevice: ProvisionDeviceUseCase
    ) {
        self.blueprintRepository = blueprintRepository
        self.fleetRepository = fleetRepository
        self.provisionDevice = provisionDevice
    }

    var selectedBlueprint: DeviceBlueprint? {
        blueprints.first { $0.id == selectedBlueprintId }
    }

    var canContinue: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            revision != nil &&
            parsedConfiguration.isValid
    }

    var registeredDevice: Device? { completedDevice ?? prepared?.device }

    func load() async {
        step = .loading
        errorMessage = nil
        do {
            async let loadedBlueprints = blueprintRepository.getBlueprints()
            async let loadedFleets = fleetRepository.getFleets()
            blueprints = try await loadedBlueprints.filter { $0.latestRevision != nil }
            fleets = try await loadedFleets
            guard let first = blueprints.first else {
                errorMessage = "Publish a device blueprint before provisioning a device."
                step = .configure
                return
            }
            selectedBlueprintId = first.id
            revision = try await blueprintRepository.getLatestRevision(blueprintId: first.id)
            step = .configure
        } catch {
            errorMessage = error.localizedDescription
            step = .configure
            logger.error("Provisioning catalog load failed: \(error.localizedDescription, privacy: .public)")
        }
    }

    func selectBlueprint(_ id: String) async {
        guard id != selectedBlueprintId || revision == nil else { return }
        selectedBlueprintId = id
        revision = nil
        configurationText = ""
        errorMessage = nil
        do {
            let loadedRevision = try await blueprintRepository.getLatestRevision(blueprintId: id)
            guard selectedBlueprintId == id else { return }
            revision = loadedRevision
        } catch {
            guard selectedBlueprintId == id else { return }
            errorMessage = error.localizedDescription
            logger.error("Blueprint revision load failed: \(error.localizedDescription, privacy: .public)")
        }
    }

    func beginScanning() {
        guard canContinue else { return }
        errorMessage = nil
        step = .scan
    }

    func prepare(info: NearbyDeviceInfo) async {
        guard step == .scan, let revision else { return }
        errorMessage = nil
        step = .registering

        let request = CreateDeviceRequest(
            name: name.trimmingCharacters(in: .whitespacesAndNewlines),
            fleetId: selectedFleetId,
            firmware: info.firmwareVersion,
            blueprintRevisionId: revision.id,
            configuration: parsedConfiguration.value
        )

        do {
            prepared = try await provisionDevice.execute(
                request: request,
                factoryDeviceId: info.deviceId,
                bootstrapVersion: info.preferredBootstrapVersion
            )
            step = .transfer
        } catch {
            errorMessage = error.localizedDescription
            step = .scan
            logger.error("Device registration failed during provisioning: \(error.localizedDescription, privacy: .public)")
        }
    }

    func markCompleted() {
        completedDevice = prepared?.device
        prepared = nil
        errorMessage = nil
        step = .completed
    }

    func showTransferError(_ message: String) {
        errorMessage = message
    }

    func rollbackIfNeeded() async -> Bool {
        guard step != .completed, let device = prepared?.device else { return true }
        isRollingBack = true
        defer { isRollingBack = false }
        do {
            try await provisionDevice.rollback(deviceId: device.id)
            prepared = nil
            return true
        } catch {
            errorMessage = "The device could not be removed from inventory: \(error.localizedDescription)"
            return false
        }
    }

    private var parsedConfiguration: (isValid: Bool, value: AnyCodableValue?) {
        let trimmed = configurationText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return (true, nil) }
        guard let data = trimmed.data(using: .utf8),
              let value = try? JSONDecoder().decode(AnyCodableValue.self, from: data)
        else { return (false, nil) }
        return (true, value)
    }
}
