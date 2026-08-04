import Foundation

enum TelemetryTimeRange: String, CaseIterable {
    case oneHour = "1h"
    case sixHours = "6h"
    case oneDay = "24h"
    case sevenDays = "7d"

    var label: String {
        switch self {
        case .oneHour: "Last 1h"
        case .sixHours: "Last 6h"
        case .oneDay: "Last 24h"
        case .sevenDays: "Last 7d"
        }
    }

    var sinceDate: Date {
        let interval: TimeInterval = switch self {
        case .oneHour: -3600
        case .sixHours: -21600
        case .oneDay: -86400
        case .sevenDays: -604800
        }
        return Date.now.addingTimeInterval(interval)
    }

    var sinceRFC3339: String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: sinceDate)
    }
}
