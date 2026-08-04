import Foundation

struct DashboardData: Sendable {
    let stats: DashboardStats?
    let alertSummary: AlertSummary?
    let currentMetrics: CurrentMetricsResponse?
    let metricsHistory: MetricsHistoryResponse?
    let statsError: String?
    let metricsError: String?
}

struct GetDashboardDataUseCase: Sendable {
    private let dashboardRepository: any DashboardRepository
    private let alertRepository: any AlertRepository
    private let metricsRepository: any MetricsRepository

    init(dashboardRepository: any DashboardRepository, alertRepository: any AlertRepository, metricsRepository: any MetricsRepository) {
        self.dashboardRepository = dashboardRepository
        self.alertRepository = alertRepository
        self.metricsRepository = metricsRepository
    }

    private enum FetchResult: Sendable {
        case stats(DashboardStats?, String?)
        case alertSummary(AlertSummary?)
        case currentMetrics(CurrentMetricsResponse?, String?)
        case metricsHistory(MetricsHistoryResponse?)
    }

    func execute(metricsSince: String, metricsResolution: Int) async -> DashboardData {
        let results = await withTaskGroup(of: FetchResult.self) { group -> [FetchResult] in
            group.addTask {
                do { return .stats(try await dashboardRepository.getStats(), nil) }
                catch { return .stats(nil, error.localizedDescription) }
            }
            group.addTask {
                do { return .alertSummary(try await alertRepository.getAlertSummary()) }
                catch { return .alertSummary(nil) }
            }
            group.addTask {
                do { return .currentMetrics(try await metricsRepository.getCurrentMetrics(), nil) }
                catch { return .currentMetrics(nil, error.localizedDescription) }
            }
            group.addTask {
                do { return .metricsHistory(try await metricsRepository.getMetricsHistory(since: metricsSince, resolution: metricsResolution)) }
                catch { return .metricsHistory(nil) }
            }
            var collected: [FetchResult] = []
            for await result in group { collected.append(result) }
            return collected
        }

        var stats: DashboardStats?
        var alertSummary: AlertSummary?
        var currentMetrics: CurrentMetricsResponse?
        var metricsHistory: MetricsHistoryResponse?
        var statsError: String?
        var metricsError: String?

        for result in results {
            switch result {
            case .stats(let s, let e):
                stats = s; statsError = e
            case .alertSummary(let a):
                alertSummary = a
            case .currentMetrics(let m, let e):
                currentMetrics = m; metricsError = e
            case .metricsHistory(let h):
                metricsHistory = h
            }
        }

        return DashboardData(
            stats: stats, alertSummary: alertSummary,
            currentMetrics: currentMetrics, metricsHistory: metricsHistory,
            statsError: statsError, metricsError: metricsError
        )
    }
}
