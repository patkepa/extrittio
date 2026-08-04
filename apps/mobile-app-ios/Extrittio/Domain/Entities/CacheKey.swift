import Foundation

enum CacheKey: Hashable, Sendable {
    case devices
    case device(id: String)
    case dashboardStats
    case alertSummary
    case telemetry(deviceId: String)
    case shadow(deviceId: String)
    case commands(deviceId: String)
    case config(deviceId: String)
    case logs(deviceId: String)
    case alerts(deviceId: String)
    case otaDeployments(deviceId: String)
    case firmware
    case fleets
    case deviceTypes
    case location(deviceId: String)
    case metrics
    case metricsHistory
    case rules
    case zones

    var stringValue: String {
        switch self {
        case .devices: "devices"
        case .device(let id): "device:\(id)"
        case .dashboardStats: "dashboard_stats"
        case .alertSummary: "alert_summary"
        case .telemetry(let id): "telemetry:\(id)"
        case .shadow(let id): "shadow:\(id)"
        case .commands(let id): "commands:\(id)"
        case .config(let id): "config:\(id)"
        case .logs(let id): "logs:\(id)"
        case .alerts(let id): "alerts:\(id)"
        case .otaDeployments(let id): "ota_deployments:\(id)"
        case .firmware: "firmware"
        case .fleets: "fleets"
        case .deviceTypes: "device_types"
        case .location(let id): "location:\(id)"
        case .metrics: "metrics"
        case .metricsHistory: "metrics_history"
        case .rules: "rules"
        case .zones: "zones"
        }
    }
}
