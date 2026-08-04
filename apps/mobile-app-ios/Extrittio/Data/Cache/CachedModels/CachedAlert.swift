import Foundation
import SwiftData

@Model
final class CachedAlert {
    @Attribute(.unique) var id: String
    var ruleId: String?
    var deviceId: String
    var severity: String
    var status: String
    var message: String
    var triggeredValue: String?
    var resolvedAt: String?
    var acknowledgedAt: String?
    var createdAt: String
    var cachedAt: Date

    init(id: String, ruleId: String?, deviceId: String, severity: String, status: String,
         message: String, triggeredValue: String?, resolvedAt: String?,
         acknowledgedAt: String?, createdAt: String, cachedAt: Date = Date()) {
        self.id = id; self.ruleId = ruleId; self.deviceId = deviceId
        self.severity = severity; self.status = status; self.message = message
        self.triggeredValue = triggeredValue; self.resolvedAt = resolvedAt
        self.acknowledgedAt = acknowledgedAt; self.createdAt = createdAt; self.cachedAt = cachedAt
    }

    func toDomain() -> Alert {
        Alert(id: id, ruleId: ruleId, deviceId: deviceId, severity: severity,
              status: status, message: message, triggeredValue: triggeredValue,
              resolvedAt: resolvedAt, acknowledgedAt: acknowledgedAt, createdAt: createdAt)
    }

    static func from(_ alert: Alert) -> CachedAlert {
        CachedAlert(id: alert.id, ruleId: alert.ruleId, deviceId: alert.deviceId,
                    severity: alert.severity, status: alert.status, message: alert.message,
                    triggeredValue: alert.triggeredValue, resolvedAt: alert.resolvedAt,
                    acknowledgedAt: alert.acknowledgedAt, createdAt: alert.createdAt)
    }
}
