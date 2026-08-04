import Foundation

struct Alert: Codable, Sendable, Identifiable {
    let id: String
    let ruleId: String?
    let deviceId: String
    let severity: String
    let status: String
    let message: String
    let triggeredValue: String?
    let resolvedAt: String?
    let acknowledgedAt: String?
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id, severity, status, message
        case ruleId = "rule_id"
        case deviceId = "device_id"
        case triggeredValue = "triggered_value"
        case resolvedAt = "resolved_at"
        case acknowledgedAt = "acknowledged_at"
        case createdAt = "created_at"
    }

    var isActive: Bool { status == "active" }
    var isAcknowledged: Bool { status == "acknowledged" }
    var isResolved: Bool { status == "resolved" }

    var severityColor: String {
        switch severity {
        case "critical": "red"
        case "warning": "orange"
        case "info": "blue"
        default: "gray"
        }
    }
}
