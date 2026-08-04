import Foundation

enum LocationTimeRange: String, CaseIterable {
    case fifteenMinutes = "15m"
    case oneHour = "1h"
    case sixHours = "6h"
    case oneDay = "24h"
    case sevenDays = "7d"
    case thirtyDays = "30d"

    var label: String {
        switch self {
        case .fifteenMinutes: "15m"
        case .oneHour: "1h"
        case .sixHours: "6h"
        case .oneDay: "24h"
        case .sevenDays: "7d"
        case .thirtyDays: "30d"
        }
    }

    var sinceDate: Date {
        let interval: TimeInterval = switch self {
        case .fifteenMinutes: -900
        case .oneHour: -3600
        case .sixHours: -21600
        case .oneDay: -86400
        case .sevenDays: -604800
        case .thirtyDays: -2592000
        }
        return Date.now.addingTimeInterval(interval)
    }

    var sinceRFC3339: String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: sinceDate)
    }
}
