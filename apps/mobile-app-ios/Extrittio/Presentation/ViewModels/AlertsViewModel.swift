import Foundation
import os

@Observable
@MainActor
final class AlertsViewModel {
    var alertsState: ViewState<[Alert]> = .loading
    var totalAlerts = 0
    var statusFilter: String?
    var severityFilter: String?

    private let getAlertsUseCase: GetAlertsUseCase
    private let acknowledgeAlertUseCase: AcknowledgeAlertUseCase
    private let resolveAlertUseCase: ResolveAlertUseCase
    private let cacheMetadata: any CacheMetadataProvider
    private let connectionMonitor: ConnectionMonitor
    private let logger = Logger(subsystem: "com.extrittio", category: "Alerts")
    private let pageSize = 50

    init(getAlertsUseCase: GetAlertsUseCase, acknowledgeAlertUseCase: AcknowledgeAlertUseCase, resolveAlertUseCase: ResolveAlertUseCase, cacheMetadata: any CacheMetadataProvider, connectionMonitor: ConnectionMonitor) {
        self.getAlertsUseCase = getAlertsUseCase
        self.acknowledgeAlertUseCase = acknowledgeAlertUseCase
        self.resolveAlertUseCase = resolveAlertUseCase
        self.cacheMetadata = cacheMetadata
        self.connectionMonitor = connectionMonitor
    }

    func load() async {
        alertsState = .loading
        do {
            let response = try await getAlertsUseCase.execute(status: statusFilter, severity: severityFilter, limit: pageSize)
            totalAlerts = response.total
            if response.data.isEmpty {
                alertsState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .alertSummary) {
                alertsState = .cached(response.data, lastUpdated: lastUpdated)
            } else {
                alertsState = .loaded(response.data)
            }
        } catch {
            alertsState = .error(error)
            logger.error("Alerts load failed: \(error)")
        }
    }

    func acknowledgeAlert(id: String) async -> Bool {
        do {
            try await acknowledgeAlertUseCase.execute(id: id)
            await load()
            return true
        } catch {
            logger.error("Acknowledge alert failed: \(error)")
            return false
        }
    }

    func resolveAlert(id: String) async -> Bool {
        do {
            try await resolveAlertUseCase.execute(id: id)
            await load()
            return true
        } catch {
            logger.error("Resolve alert failed: \(error)")
            return false
        }
    }
}
