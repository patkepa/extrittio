import Foundation
import os

struct OTAData: Sendable {
    let deployments: [OtaDeployment]
    let firmware: [FirmwareUpdate]
}

@Observable
@MainActor
final class DeviceDetailViewModel {
    let deviceId: String

    // Lazy loading flags
    private var hasLoadedTelemetry = false
    private var hasLoadedShadow = false
    private var hasLoadedCommands = false
    private var hasLoadedConfig = false
    private var hasLoadedLogs = false
    private var hasLoadedOTA = false
    private var hasLoadedAlerts = false
    private var hasLoadedLocation = false

    // Telemetry time range
    var telemetryTimeRange: TelemetryTimeRange = .oneDay

    // Location time range
    var locationTimeRange: LocationTimeRange = .oneDay

    // Log level filter
    var logLevelFilter: String?

    // MARK: - ViewState Properties

    var deviceState: ViewState<Device> = .loading
    var telemetryState: ViewState<[TelemetryRecord]> = .loading
    var shadowState: ViewState<DeviceShadow?> = .loading
    var commandsState: ViewState<[CommandRecord]> = .loading
    var configState: ViewState<[String: AnyCodableValue]> = .loading
    var logsState: ViewState<[DeviceLog]> = .loading
    var otaState: ViewState<OTAData> = .loading
    var alertsState: ViewState<[Alert]> = .loading
    var locationState: ViewState<DeviceLocation?> = .loading
    var locationTrailState: ViewState<[TelemetryRecord]> = .loading

    // MARK: - Compatibility Computed Properties

    var device: Device? {
        get { deviceState.data }
        set {
            if let newValue {
                deviceState = .loaded(newValue)
            } else {
                deviceState = .empty
            }
        }
    }

    var isLoadingDevice: Bool { deviceState.isLoading }

    var telemetry: [TelemetryRecord] { telemetryState.data ?? [] }
    var isLoadingTelemetry: Bool { telemetryState.isLoading }

    var shadow: DeviceShadow? {
        get { shadowState.data ?? nil }
        set {
            if let newValue {
                shadowState = .loaded(newValue)
            } else {
                shadowState = .loaded(nil)
            }
        }
    }
    var isLoadingShadow: Bool { shadowState.isLoading }

    var commands: [CommandRecord] {
        get { commandsState.data ?? [] }
        set { commandsState = .loaded(newValue) }
    }
    var isLoadingCommands: Bool { commandsState.isLoading }

    var config: [String: AnyCodableValue] {
        get { configState.data ?? [:] }
        set { configState = .loaded(newValue) }
    }
    var isLoadingConfig: Bool { configState.isLoading }

    var logs: [DeviceLog] { logsState.data ?? [] }
    var isLoadingLogs: Bool { logsState.isLoading }

    var deployments: [OtaDeployment] { otaState.data?.deployments ?? [] }
    var availableFirmware: [FirmwareUpdate] { otaState.data?.firmware ?? [] }
    var isLoadingOTA: Bool { otaState.isLoading }

    var alerts: [Alert] { alertsState.data ?? [] }
    var isLoadingAlerts: Bool { alertsState.isLoading }

    var latestLocation: DeviceLocation? { locationState.data ?? nil }
    var isLoadingLocation: Bool { locationState.isLoading }
    var locationTrail: [TelemetryRecord] { locationTrailState.data ?? [] }
    var isLoadingLocationTrail: Bool { locationTrailState.isLoading }

    var errorMessage: String? {
        deviceState.errorMessage
            ?? telemetryState.errorMessage
            ?? shadowState.errorMessage
            ?? commandsState.errorMessage
            ?? configState.errorMessage
            ?? logsState.errorMessage
            ?? otaState.errorMessage
            ?? alertsState.errorMessage
    }

    // Use Cases
    private let getDeviceUseCase: GetDeviceUseCase
    private let deleteDeviceUseCase: DeleteDeviceUseCase
    private let restartDeviceUseCase: RestartDeviceUseCase
    private let getTelemetryUseCase: GetTelemetryUseCase
    private let getShadowUseCase: GetShadowUseCase
    private let updateDesiredStateUseCase: UpdateDesiredStateUseCase
    private let deleteShadowUseCase: DeleteShadowUseCase
    private let getCommandsUseCase: GetCommandsUseCase
    private let sendCommandUseCase: SendCommandUseCase
    private let getDeviceConfigUseCase: GetDeviceConfigUseCase
    private let updateDeviceConfigUseCase: UpdateDeviceConfigUseCase
    private let getDeviceLogsUseCase: GetDeviceLogsUseCase
    private let getOTADataUseCase: GetOTADataUseCase
    private let triggerOTAUseCase: TriggerOTAUseCase
    private let getAlertsUseCase: GetAlertsUseCase
    private let acknowledgeAlertUseCase: AcknowledgeAlertUseCase
    private let resolveAlertUseCase: ResolveAlertUseCase
    private let getDeviceLatestLocationUseCase: GetDeviceLatestLocationUseCase

    private let cacheMetadata: any CacheMetadataProvider
    private let connectionMonitor: ConnectionMonitor
    private let logger = Logger(subsystem: "com.extrittio", category: "DeviceDetail")

    init(
        deviceId: String,
        getDeviceUseCase: GetDeviceUseCase,
        deleteDeviceUseCase: DeleteDeviceUseCase,
        restartDeviceUseCase: RestartDeviceUseCase,
        getTelemetryUseCase: GetTelemetryUseCase,
        getShadowUseCase: GetShadowUseCase,
        updateDesiredStateUseCase: UpdateDesiredStateUseCase,
        deleteShadowUseCase: DeleteShadowUseCase,
        getCommandsUseCase: GetCommandsUseCase,
        sendCommandUseCase: SendCommandUseCase,
        getDeviceConfigUseCase: GetDeviceConfigUseCase,
        updateDeviceConfigUseCase: UpdateDeviceConfigUseCase,
        getDeviceLogsUseCase: GetDeviceLogsUseCase,
        getOTADataUseCase: GetOTADataUseCase,
        triggerOTAUseCase: TriggerOTAUseCase,
        getAlertsUseCase: GetAlertsUseCase,
        acknowledgeAlertUseCase: AcknowledgeAlertUseCase,
        resolveAlertUseCase: ResolveAlertUseCase,
        getDeviceLatestLocationUseCase: GetDeviceLatestLocationUseCase,
        cacheMetadata: any CacheMetadataProvider,
        connectionMonitor: ConnectionMonitor
    ) {
        self.deviceId = deviceId
        self.getDeviceUseCase = getDeviceUseCase
        self.deleteDeviceUseCase = deleteDeviceUseCase
        self.restartDeviceUseCase = restartDeviceUseCase
        self.getTelemetryUseCase = getTelemetryUseCase
        self.getShadowUseCase = getShadowUseCase
        self.updateDesiredStateUseCase = updateDesiredStateUseCase
        self.deleteShadowUseCase = deleteShadowUseCase
        self.getCommandsUseCase = getCommandsUseCase
        self.sendCommandUseCase = sendCommandUseCase
        self.getDeviceConfigUseCase = getDeviceConfigUseCase
        self.updateDeviceConfigUseCase = updateDeviceConfigUseCase
        self.getDeviceLogsUseCase = getDeviceLogsUseCase
        self.getOTADataUseCase = getOTADataUseCase
        self.triggerOTAUseCase = triggerOTAUseCase
        self.getAlertsUseCase = getAlertsUseCase
        self.acknowledgeAlertUseCase = acknowledgeAlertUseCase
        self.resolveAlertUseCase = resolveAlertUseCase
        self.getDeviceLatestLocationUseCase = getDeviceLatestLocationUseCase
        self.cacheMetadata = cacheMetadata
        self.connectionMonitor = connectionMonitor
    }

    // MARK: - Device

    func loadDevice() async {
        deviceState = .loading
        do {
            let loadedDevice = try await getDeviceUseCase.execute(id: deviceId)
            if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .device(id: deviceId)) {
                deviceState = .cached(loadedDevice, lastUpdated: lastUpdated)
            } else {
                deviceState = .loaded(loadedDevice)
            }
        } catch {
            deviceState = .error(error)
            logger.error("Load device failed: \(error)")
        }
    }

    func restartDevice() async -> Bool {
        do {
            try await restartDeviceUseCase.execute(id: deviceId)
            return true
        } catch {
            logger.error("Restart device failed: \(error)")
            return false
        }
    }

    func deleteDevice() async -> Bool {
        do {
            try await deleteDeviceUseCase.execute(id: deviceId)
            return true
        } catch {
            logger.error("Delete device failed: \(error)")
            return false
        }
    }

    // MARK: - Telemetry

    func loadTelemetry(forceRefresh: Bool = false) async {
        guard forceRefresh || !hasLoadedTelemetry else { return }
        telemetryState = .loading
        do {
            let records = try await getTelemetryUseCase.execute(
                deviceId: deviceId, limit: 1000, since: telemetryTimeRange.sinceRFC3339
            )
            if records.isEmpty {
                telemetryState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .telemetry(deviceId: deviceId)) {
                telemetryState = .cached(records, lastUpdated: lastUpdated)
            } else {
                telemetryState = .loaded(records)
            }
            hasLoadedTelemetry = true
        } catch {
            telemetryState = .error(error)
            logger.error("Load telemetry failed: \(error)")
        }
    }

    func resetTelemetryAndReload() async {
        hasLoadedTelemetry = false
        await loadTelemetry()
    }

    // MARK: - Shadow

    func loadShadow(forceRefresh: Bool = false) async {
        guard forceRefresh || !hasLoadedShadow else { return }
        shadowState = .loading
        do {
            let loadedShadow = try await getShadowUseCase.execute(deviceId: deviceId)
            if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .shadow(deviceId: deviceId)) {
                shadowState = .cached(loadedShadow, lastUpdated: lastUpdated)
            } else {
                shadowState = .loaded(loadedShadow)
            }
            hasLoadedShadow = true
        } catch {
            shadowState = .error(error)
            logger.error("Load shadow failed: \(error)")
        }
    }

    func updateDesiredState(_ desired: [String: AnyCodableValue]) async -> Bool {
        do {
            let updatedShadow = try await updateDesiredStateUseCase.execute(deviceId: deviceId, desired: desired)
            shadowState = .loaded(updatedShadow)
            return true
        } catch {
            logger.error("Update desired state failed: \(error)")
            return false
        }
    }

    func deleteShadow() async -> Bool {
        do {
            try await deleteShadowUseCase.execute(deviceId: deviceId)
            shadowState = .loaded(nil)
            return true
        } catch {
            logger.error("Delete shadow failed: \(error)")
            return false
        }
    }

    // MARK: - Commands

    func loadCommands(forceRefresh: Bool = false) async {
        guard forceRefresh || !hasLoadedCommands else { return }
        commandsState = .loading
        do {
            let loadedCommands = try await getCommandsUseCase.execute(deviceId: deviceId)
            if loadedCommands.isEmpty {
                commandsState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .commands(deviceId: deviceId)) {
                commandsState = .cached(loadedCommands, lastUpdated: lastUpdated)
            } else {
                commandsState = .loaded(loadedCommands)
            }
            hasLoadedCommands = true
        } catch {
            commandsState = .error(error)
            logger.error("Load commands failed: \(error)")
        }
    }

    func sendCommand(command: String, params: [String: AnyCodableValue]?) async -> Bool {
        do {
            let record = try await sendCommandUseCase.execute(deviceId: deviceId, command: command, params: params)
            var current = commandsState.data ?? []
            current.insert(record, at: 0)
            commandsState = .loaded(current)
            return true
        } catch {
            logger.error("Send command failed: \(error)")
            return false
        }
    }

    // MARK: - Config

    func loadConfig(forceRefresh: Bool = false) async {
        guard forceRefresh || !hasLoadedConfig else { return }
        configState = .loading
        do {
            let loadedConfig = try await getDeviceConfigUseCase.execute(deviceId: deviceId)
            if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .config(deviceId: deviceId)) {
                configState = .cached(loadedConfig, lastUpdated: lastUpdated)
            } else {
                configState = .loaded(loadedConfig)
            }
            hasLoadedConfig = true
        } catch {
            configState = .error(error)
            logger.error("Load config failed: \(error)")
        }
    }

    func updateConfig(_ newConfig: [String: AnyCodableValue]) async -> Bool {
        do {
            let updatedConfig = try await updateDeviceConfigUseCase.execute(deviceId: deviceId, config: newConfig)
            configState = .loaded(updatedConfig)
            return true
        } catch {
            logger.error("Update config failed: \(error)")
            return false
        }
    }

    // MARK: - Logs

    func loadLogs(forceRefresh: Bool = false) async {
        guard forceRefresh || !hasLoadedLogs else { return }
        logsState = .loading
        do {
            let loadedLogs = try await getDeviceLogsUseCase.execute(deviceId: deviceId, level: logLevelFilter)
            if loadedLogs.isEmpty {
                logsState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .logs(deviceId: deviceId)) {
                logsState = .cached(loadedLogs, lastUpdated: lastUpdated)
            } else {
                logsState = .loaded(loadedLogs)
            }
            hasLoadedLogs = true
        } catch {
            logsState = .error(error)
            logger.error("Load logs failed: \(error)")
        }
    }

    func resetLogsAndReload() async {
        hasLoadedLogs = false
        await loadLogs()
    }

    // MARK: - OTA

    func loadOTA(forceRefresh: Bool = false) async {
        guard forceRefresh || !hasLoadedOTA else { return }
        otaState = .loading
        do {
            let result = try await getOTADataUseCase.execute(deviceId: deviceId, deviceTypeId: device?.deviceTypeId)
            let otaData = OTAData(deployments: result.deployments, firmware: result.firmware)
            if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .otaDeployments(deviceId: deviceId)) {
                otaState = .cached(otaData, lastUpdated: lastUpdated)
            } else {
                otaState = .loaded(otaData)
            }
            hasLoadedOTA = true
        } catch {
            otaState = .error(error)
            logger.error("Load OTA failed: \(error)")
        }
    }

    func triggerOTA(firmwareUpdateId: Int) async -> Bool {
        do {
            try await triggerOTAUseCase.execute(deviceId: deviceId, firmwareUpdateId: firmwareUpdateId)
            await loadOTA(forceRefresh: true)
            return true
        } catch {
            logger.error("Trigger OTA failed: \(error)")
            return false
        }
    }

    // MARK: - Alerts

    func loadAlerts(forceRefresh: Bool = false) async {
        guard forceRefresh || !hasLoadedAlerts else { return }
        alertsState = .loading
        do {
            let response = try await getAlertsUseCase.execute(deviceId: deviceId)
            if response.data.isEmpty {
                alertsState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .alerts(deviceId: deviceId)) {
                alertsState = .cached(response.data, lastUpdated: lastUpdated)
            } else {
                alertsState = .loaded(response.data)
            }
            hasLoadedAlerts = true
        } catch {
            alertsState = .error(error)
            logger.error("Load alerts failed: \(error)")
        }
    }

    func acknowledgeAlert(id: String) async -> Bool {
        do {
            try await acknowledgeAlertUseCase.execute(id: id)
            await loadAlerts(forceRefresh: true)
            return true
        } catch {
            logger.error("Acknowledge alert failed: \(error)")
            return false
        }
    }

    func resolveAlert(id: String) async -> Bool {
        do {
            try await resolveAlertUseCase.execute(id: id)
            await loadAlerts(forceRefresh: true)
            return true
        } catch {
            logger.error("Resolve alert failed: \(error)")
            return false
        }
    }

    // MARK: - Location

    func loadLocation(forceRefresh: Bool = false) async {
        guard forceRefresh || !hasLoadedLocation else { return }
        locationState = .loading
        locationTrailState = .loading
        do {
            async let locationResult = getDeviceLatestLocationUseCase.execute(deviceId: deviceId)
            async let trailResult = getTelemetryUseCase.execute(
                deviceId: deviceId, limit: 1000, since: locationTimeRange.sinceRFC3339
            )
            let location = try await locationResult
            if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .location(deviceId: deviceId)) {
                locationState = .cached(location, lastUpdated: lastUpdated)
            } else {
                locationState = .loaded(location)
            }
            let trail = try await trailResult
            let locationRecords = trail.filter { $0.hasLocation }
            if locationRecords.isEmpty {
                locationTrailState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .telemetry(deviceId: deviceId)) {
                locationTrailState = .cached(locationRecords, lastUpdated: lastUpdated)
            } else {
                locationTrailState = .loaded(locationRecords)
            }
            hasLoadedLocation = true
        } catch {
            locationState = .error(error)
            locationTrailState = .error(error)
            logger.error("Load location failed: \(error)")
        }
    }

    func resetLocationAndReload() async {
        hasLoadedLocation = false
        await loadLocation()
    }
}
