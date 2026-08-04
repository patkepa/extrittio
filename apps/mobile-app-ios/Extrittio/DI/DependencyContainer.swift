import Foundation
import SwiftData

@MainActor
final class DependencyContainer {
    private let apiClient: APIClient
    let serverAddressProvider: any ServerAddressProvider
    let cacheManager: CacheManager
    let connectionMonitor: ConnectionMonitor
    private let authSession: AuthSession

    private let _authRepository: any AuthRepository
    private let _deviceRepository: any DeviceRepository
    private let _telemetryRepository: any TelemetryRepository
    private let _shadowRepository: any DeviceShadowRepository
    private let _commandRepository: any CommandRepository
    private let _configRepository: any DeviceConfigRepository
    private let _logRepository: any DeviceLogRepository
    private let _otaRepository: any OTARepository
    private let _alertRepository: any AlertRepository
    private let _dashboardRepository: any DashboardRepository
    private let _metricsRepository: any MetricsRepository
    private let _ruleRepository: any RuleRepository
    private let _firmwareRepository: any FirmwareRepository
    private let _fleetRepository: any FleetRepository
    private let _deviceTypeRepository: any DeviceTypeRepository
    private let _zoneRepository: any ZoneRepository
    private let _userRepository: any UserRepository
    private let _roleRepository: any RoleRepository
    private let _apiKeyRepository: any ApiKeyRepository
    private let _certificateRepository: any CertificateRepository

    init() {
        // SwiftData setup
        let schema = Schema([
            CachedDevice.self, CachedDashboardStats.self, CachedAlertSummary.self,
            CachedTelemetryRecord.self, CachedDeviceShadow.self, CachedCommandRecord.self,
            CachedDeviceConfig.self, CachedDeviceLog.self, CachedAlert.self,
            CachedOtaDeployment.self, CachedFirmwareUpdate.self, CachedFleet.self,
            CachedDeviceType.self, CachedDeviceLocation.self, CachedMetrics.self,
            CachedRule.self, CachedZone.self, CacheEntry.self
        ])
        let modelConfig = ModelConfiguration(isStoredInMemoryOnly: false)
        try? FileManager.default.createDirectory(
            at: modelConfig.url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let container: ModelContainer
        do {
            container = try ModelContainer(for: schema, configurations: [modelConfig])
        } catch {
            // Schema mismatch — delete store and retry
            let storeURL = modelConfig.url
            try? FileManager.default.removeItem(at: storeURL)
            container = try! ModelContainer(for: schema, configurations: [modelConfig])
        }
        let cache = CacheManager(modelContainer: container)
        self.cacheManager = cache

        let authSession = AuthSession()
        self.authSession = authSession

        let provider = UserDefaultsServerAddressProvider()
        self.serverAddressProvider = provider
        let client = APIClient(
            serverAddressProvider: provider,
            unauthorizedHandler: {
                await cache.clearAll()
                await authSession.expire(message: "Session expired. Please log in again.")
            }
        )
        self.apiClient = client
        self.connectionMonitor = ConnectionMonitor(serverAddressProvider: provider)

        // Auth — no caching
        self._authRepository = AuthRepositoryImpl(apiClient: client)

        // All other repos — caching wrappers
        self._deviceRepository = CachingDeviceRepository(remote: DeviceRepositoryImpl(apiClient: client), cacheManager: cache)
        self._telemetryRepository = CachingTelemetryRepository(remote: TelemetryRepositoryImpl(apiClient: client), cacheManager: cache)
        self._shadowRepository = CachingDeviceShadowRepository(remote: DeviceShadowRepositoryImpl(apiClient: client), cacheManager: cache)
        self._commandRepository = CachingCommandRepository(remote: CommandRepositoryImpl(apiClient: client), cacheManager: cache)
        self._configRepository = CachingDeviceConfigRepository(remote: DeviceConfigRepositoryImpl(apiClient: client), cacheManager: cache)
        self._logRepository = CachingDeviceLogRepository(remote: DeviceLogRepositoryImpl(apiClient: client), cacheManager: cache)
        self._otaRepository = CachingOTARepository(remote: OTARepositoryImpl(apiClient: client), cacheManager: cache)
        self._alertRepository = CachingAlertRepository(remote: AlertRepositoryImpl(apiClient: client), cacheManager: cache)
        self._dashboardRepository = CachingDashboardRepository(remote: DashboardRepositoryImpl(apiClient: client), cacheManager: cache)
        self._metricsRepository = CachingMetricsRepository(remote: MetricsRepositoryImpl(apiClient: client), cacheManager: cache)
        self._ruleRepository = CachingRuleRepository(remote: RuleRepositoryImpl(apiClient: client), cacheManager: cache)
        self._firmwareRepository = CachingFirmwareRepository(remote: FirmwareRepositoryImpl(apiClient: client), cacheManager: cache)
        self._fleetRepository = CachingFleetRepository(remote: FleetRepositoryImpl(apiClient: client), cacheManager: cache)
        self._deviceTypeRepository = CachingDeviceTypeRepository(remote: DeviceTypeRepositoryImpl(apiClient: client), cacheManager: cache)
        self._zoneRepository = CachingZoneRepository(remote: ZoneRepositoryImpl(apiClient: client), cacheManager: cache)
        self._userRepository = UserRepositoryImpl(apiClient: client)
        self._roleRepository = RoleRepositoryImpl(apiClient: client)
        self._apiKeyRepository = ApiKeyRepositoryImpl(apiClient: client)
        self._certificateRepository = CertificateRepositoryImpl(apiClient: client)
    }

    // MARK: - Use Case Factories

    func makeLoginUseCase() -> LoginUseCase { LoginUseCase(repository: _authRepository) }
    func makeLogoutUseCase() -> LogoutUseCase { LogoutUseCase(repository: _authRepository) }
    func makeCheckAuthUseCase() -> CheckAuthUseCase { CheckAuthUseCase(repository: _authRepository) }
    func makeGetDashboardDataUseCase() -> GetDashboardDataUseCase {
        GetDashboardDataUseCase(dashboardRepository: _dashboardRepository, alertRepository: _alertRepository, metricsRepository: _metricsRepository)
    }
    func makeGetDevicesUseCase() -> GetDevicesUseCase { GetDevicesUseCase(repository: _deviceRepository) }
    func makeGetFleetsUseCase() -> GetFleetsUseCase { GetFleetsUseCase(repository: _fleetRepository) }
    func makeGetDeviceTypesUseCase() -> GetDeviceTypesUseCase { GetDeviceTypesUseCase(repository: _deviceTypeRepository) }
    func makeGetDeviceUseCase() -> GetDeviceUseCase { GetDeviceUseCase(repository: _deviceRepository) }
    func makeCreateDeviceUseCase() -> CreateDeviceUseCase { CreateDeviceUseCase(repository: _deviceRepository) }
    func makeUpdateDeviceUseCase() -> UpdateDeviceUseCase { UpdateDeviceUseCase(repository: _deviceRepository) }
    func makeDeleteDeviceUseCase() -> DeleteDeviceUseCase { DeleteDeviceUseCase(repository: _deviceRepository) }
    func makeRestartDeviceUseCase() -> RestartDeviceUseCase { RestartDeviceUseCase(repository: _deviceRepository) }
    func makeGetTelemetryUseCase() -> GetTelemetryUseCase { GetTelemetryUseCase(repository: _telemetryRepository) }
    func makeGetShadowUseCase() -> GetShadowUseCase { GetShadowUseCase(repository: _shadowRepository) }
    func makeUpdateDesiredStateUseCase() -> UpdateDesiredStateUseCase { UpdateDesiredStateUseCase(repository: _shadowRepository) }
    func makeDeleteShadowUseCase() -> DeleteShadowUseCase { DeleteShadowUseCase(repository: _shadowRepository) }
    func makeGetCommandsUseCase() -> GetCommandsUseCase { GetCommandsUseCase(repository: _commandRepository) }
    func makeSendCommandUseCase() -> SendCommandUseCase { SendCommandUseCase(repository: _commandRepository) }
    func makeGetDeviceConfigUseCase() -> GetDeviceConfigUseCase { GetDeviceConfigUseCase(repository: _configRepository) }
    func makeUpdateDeviceConfigUseCase() -> UpdateDeviceConfigUseCase { UpdateDeviceConfigUseCase(repository: _configRepository) }
    func makeGetDeviceLogsUseCase() -> GetDeviceLogsUseCase { GetDeviceLogsUseCase(repository: _logRepository) }
    func makeGetOTADataUseCase() -> GetOTADataUseCase { GetOTADataUseCase(repository: _otaRepository) }
    func makeTriggerOTAUseCase() -> TriggerOTAUseCase { TriggerOTAUseCase(repository: _otaRepository) }
    func makeGetAlertsUseCase() -> GetAlertsUseCase { GetAlertsUseCase(repository: _alertRepository) }
    func makeAcknowledgeAlertUseCase() -> AcknowledgeAlertUseCase { AcknowledgeAlertUseCase(repository: _alertRepository) }
    func makeResolveAlertUseCase() -> ResolveAlertUseCase { ResolveAlertUseCase(repository: _alertRepository) }
    func makeGetRulesUseCase() -> GetRulesUseCase { GetRulesUseCase(repository: _ruleRepository) }
    func makeToggleRuleUseCase() -> ToggleRuleUseCase { ToggleRuleUseCase(repository: _ruleRepository) }
    func makeDeleteRuleUseCase() -> DeleteRuleUseCase { DeleteRuleUseCase(repository: _ruleRepository) }
    func makeGetFirmwareListUseCase() -> GetFirmwareListUseCase { GetFirmwareListUseCase(firmwareRepository: _firmwareRepository, deviceTypeRepository: _deviceTypeRepository) }
    func makeDeleteFirmwareUseCase() -> DeleteFirmwareUseCase { DeleteFirmwareUseCase(repository: _firmwareRepository) }
    func makeGetZonesUseCase() -> GetZonesUseCase { GetZonesUseCase(repository: _zoneRepository) }
    func makeGetDeviceLatestLocationUseCase() -> GetDeviceLatestLocationUseCase { GetDeviceLatestLocationUseCase(repository: _deviceRepository) }

    // MARK: - ViewModel Factories

    func makeSettingsViewModel() -> SettingsViewModel {
        SettingsViewModel(cacheManager: cacheManager)
    }

    func makeAdminSettingsViewModel() -> AdminSettingsViewModel {
        AdminSettingsViewModel(
            userRepository: _userRepository,
            roleRepository: _roleRepository,
            apiKeyRepository: _apiKeyRepository,
            certificateRepository: _certificateRepository,
            deviceRepository: _deviceRepository,
            fleetRepository: _fleetRepository,
            deviceTypeRepository: _deviceTypeRepository
        )
    }

    func makeAuthViewModel() -> AuthViewModel {
        AuthViewModel(
            loginUseCase: makeLoginUseCase(),
            logoutUseCase: makeLogoutUseCase(),
            checkAuthUseCase: makeCheckAuthUseCase(),
            cacheManager: cacheManager,
            session: authSession
        )
    }

    func makeDashboardViewModel() -> DashboardViewModel {
        DashboardViewModel(getDashboardDataUseCase: makeGetDashboardDataUseCase(), cacheMetadata: cacheManager, connectionMonitor: connectionMonitor)
    }

    func makeDeviceListViewModel() -> DeviceListViewModel {
        DeviceListViewModel(getDevicesUseCase: makeGetDevicesUseCase(), getFleetsUseCase: makeGetFleetsUseCase(), deleteDeviceUseCase: makeDeleteDeviceUseCase(), restartDeviceUseCase: makeRestartDeviceUseCase(), cacheMetadata: cacheManager, connectionMonitor: connectionMonitor)
    }

    func makeDeviceDetailViewModel(deviceId: String) -> DeviceDetailViewModel {
        DeviceDetailViewModel(
            deviceId: deviceId,
            getDeviceUseCase: makeGetDeviceUseCase(),
            deleteDeviceUseCase: makeDeleteDeviceUseCase(),
            restartDeviceUseCase: makeRestartDeviceUseCase(),
            getTelemetryUseCase: makeGetTelemetryUseCase(),
            getShadowUseCase: makeGetShadowUseCase(),
            updateDesiredStateUseCase: makeUpdateDesiredStateUseCase(),
            deleteShadowUseCase: makeDeleteShadowUseCase(),
            getCommandsUseCase: makeGetCommandsUseCase(),
            sendCommandUseCase: makeSendCommandUseCase(),
            getDeviceConfigUseCase: makeGetDeviceConfigUseCase(),
            updateDeviceConfigUseCase: makeUpdateDeviceConfigUseCase(),
            getDeviceLogsUseCase: makeGetDeviceLogsUseCase(),
            getOTADataUseCase: makeGetOTADataUseCase(),
            triggerOTAUseCase: makeTriggerOTAUseCase(),
            getAlertsUseCase: makeGetAlertsUseCase(),
            acknowledgeAlertUseCase: makeAcknowledgeAlertUseCase(),
            resolveAlertUseCase: makeResolveAlertUseCase(),
            getDeviceLatestLocationUseCase: makeGetDeviceLatestLocationUseCase(),
            cacheMetadata: cacheManager,
            connectionMonitor: connectionMonitor
        )
    }

    func makeAlertsViewModel() -> AlertsViewModel {
        AlertsViewModel(getAlertsUseCase: makeGetAlertsUseCase(), acknowledgeAlertUseCase: makeAcknowledgeAlertUseCase(), resolveAlertUseCase: makeResolveAlertUseCase(), cacheMetadata: cacheManager, connectionMonitor: connectionMonitor)
    }

    func makeRulesViewModel() -> RulesViewModel {
        RulesViewModel(getRulesUseCase: makeGetRulesUseCase(), toggleRuleUseCase: makeToggleRuleUseCase(), deleteRuleUseCase: makeDeleteRuleUseCase(), cacheMetadata: cacheManager, connectionMonitor: connectionMonitor)
    }

    func makeFirmwareViewModel() -> FirmwareViewModel {
        FirmwareViewModel(getFirmwareListUseCase: makeGetFirmwareListUseCase(), deleteFirmwareUseCase: makeDeleteFirmwareUseCase(), cacheMetadata: cacheManager, connectionMonitor: connectionMonitor)
    }

    func makeMapViewModel() -> MapViewModel {
        MapViewModel(getDevicesUseCase: makeGetDevicesUseCase(), getZonesUseCase: makeGetZonesUseCase(), cacheMetadata: cacheManager, connectionMonitor: connectionMonitor)
    }
}
