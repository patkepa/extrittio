import Foundation
import os

@Observable
@MainActor
final class DashboardViewModel {
    var statsState: ViewState<DashboardStats> = .loading
    var alertSummaryState: ViewState<AlertSummary> = .loading
    var metricsState: ViewState<CurrentMetricsResponse> = .loading
    var metricsHistoryState: ViewState<MetricsHistoryResponse> = .loading
    var showRefreshBar = false

    private let getDashboardDataUseCase: GetDashboardDataUseCase
    private let cacheMetadata: any CacheMetadataProvider
    private let connectionMonitor: ConnectionMonitor
    private let logger = Logger(subsystem: "com.extrittio", category: "DashboardViewModel")

    init(getDashboardDataUseCase: GetDashboardDataUseCase, cacheMetadata: any CacheMetadataProvider, connectionMonitor: ConnectionMonitor) {
        self.getDashboardDataUseCase = getDashboardDataUseCase
        self.cacheMetadata = cacheMetadata
        self.connectionMonitor = connectionMonitor
    }

    func load() async {
        let isRefresh = statsState.data != nil || metricsState.data != nil

        if !isRefresh {
            statsState = .loading
            alertSummaryState = .loading
            metricsState = .loading
            metricsHistoryState = .loading
        }

        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss"
        formatter.timeZone = .gmt
        let since = formatter.string(from: Date.now.addingTimeInterval(-3600))

        let data = await getDashboardDataUseCase.execute(metricsSince: since, metricsResolution: 60)

        let isOffline = !connectionMonitor.isOnline

        if let stats = data.stats {
            if isOffline, let lastUpdated = await cacheMetadata.lastUpdated(for: .dashboardStats) {
                statsState = .cached(stats, lastUpdated: lastUpdated)
            } else {
                statsState = .loaded(stats)
            }
        } else if let error = data.statsError {
            statsState = .error(DashboardError(message: error))
        } else {
            statsState = .empty
        }

        if let summary = data.alertSummary {
            if isOffline, let lastUpdated = await cacheMetadata.lastUpdated(for: .alertSummary) {
                alertSummaryState = .cached(summary, lastUpdated: lastUpdated)
            } else {
                alertSummaryState = .loaded(summary)
            }
        } else {
            alertSummaryState = .empty
        }

        if let metrics = data.currentMetrics {
            if isOffline, let lastUpdated = await cacheMetadata.lastUpdated(for: .metrics) {
                metricsState = .cached(metrics, lastUpdated: lastUpdated)
            } else {
                metricsState = .loaded(metrics)
            }
        } else if let error = data.metricsError {
            metricsState = .error(DashboardError(message: error))
        } else {
            metricsState = .empty
        }

        if let history = data.metricsHistory {
            if isOffline, let lastUpdated = await cacheMetadata.lastUpdated(for: .metricsHistory) {
                metricsHistoryState = .cached(history, lastUpdated: lastUpdated)
            } else {
                metricsHistoryState = .loaded(history)
            }
        } else {
            metricsHistoryState = .empty
        }

        if isRefresh {
            showRefreshBar = true
            try? await Task.sleep(for: .milliseconds(800))
            showRefreshBar = false
        }
    }

    func startAutoRefresh() async {
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(30))
            guard !Task.isCancelled else { break }
            guard connectionMonitor.isOnline else { continue }
            await load()
        }
    }
}

private struct DashboardError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
