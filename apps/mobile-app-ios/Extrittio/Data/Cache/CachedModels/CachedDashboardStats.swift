import Foundation
import SwiftData

@Model
final class CachedDashboardStats {
    @Attribute(.unique) var key: String
    var totalDevices: Int
    var activeDevices: Int
    var offlineDevices: Int
    var totalMessages: Int
    var cachedAt: Date

    init(key: String = "dashboard_stats", totalDevices: Int, activeDevices: Int,
         offlineDevices: Int, totalMessages: Int, cachedAt: Date = Date()) {
        self.key = key; self.totalDevices = totalDevices; self.activeDevices = activeDevices
        self.offlineDevices = offlineDevices; self.totalMessages = totalMessages; self.cachedAt = cachedAt
    }

    func toDomain() -> DashboardStats {
        DashboardStats(totalDevices: totalDevices, activeDevices: activeDevices,
                       offlineDevices: offlineDevices, totalMessages: totalMessages)
    }

    static func from(_ stats: DashboardStats) -> CachedDashboardStats {
        CachedDashboardStats(totalDevices: stats.totalDevices, activeDevices: stats.activeDevices,
                             offlineDevices: stats.offlineDevices, totalMessages: stats.totalMessages)
    }
}

@Model
final class CachedAlertSummary {
    @Attribute(.unique) var key: String
    var activeInfo: Int
    var activeWarning: Int
    var activeCritical: Int
    var acknowledgedInfo: Int
    var acknowledgedWarning: Int
    var acknowledgedCritical: Int
    var totalActive: Int
    var cachedAt: Date

    init(key: String = "alert_summary", activeInfo: Int, activeWarning: Int, activeCritical: Int,
         acknowledgedInfo: Int, acknowledgedWarning: Int, acknowledgedCritical: Int,
         totalActive: Int, cachedAt: Date = Date()) {
        self.key = key; self.activeInfo = activeInfo; self.activeWarning = activeWarning
        self.activeCritical = activeCritical; self.acknowledgedInfo = acknowledgedInfo
        self.acknowledgedWarning = acknowledgedWarning; self.acknowledgedCritical = acknowledgedCritical
        self.totalActive = totalActive; self.cachedAt = cachedAt
    }

    func toDomain() -> AlertSummary {
        AlertSummary(active: AlertSeverityCounts(info: activeInfo, warning: activeWarning, critical: activeCritical),
                     acknowledged: AlertSeverityCounts(info: acknowledgedInfo, warning: acknowledgedWarning, critical: acknowledgedCritical),
                     totalActive: totalActive)
    }

    static func from(_ summary: AlertSummary) -> CachedAlertSummary {
        CachedAlertSummary(activeInfo: summary.active.info, activeWarning: summary.active.warning,
                           activeCritical: summary.active.critical, acknowledgedInfo: summary.acknowledged.info,
                           acknowledgedWarning: summary.acknowledged.warning, acknowledgedCritical: summary.acknowledged.critical,
                           totalActive: summary.totalActive)
    }
}
