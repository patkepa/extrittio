import Foundation

struct Rule: Codable, Sendable, Identifiable {
    let id: String
    let name: String
    let description: String?
    let enabled: Bool
    let triggerType: String
    let targetType: String
    let targetId: String?
    let cooldownSeconds: Int
    let conditions: [RuleCondition]
    let actions: [RuleAction]
    let createdAt: String
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, name, description, enabled, conditions, actions
        case triggerType = "trigger_type"
        case targetType = "target_type"
        case targetId = "target_id"
        case cooldownSeconds = "cooldown_seconds"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }

    var triggerLabel: String {
        switch triggerType {
        case "telemetry": "Telemetry"
        case "device_status": "Device Status"
        default: triggerType
        }
    }

    var targetLabel: String {
        switch targetType {
        case "global": "All Devices"
        case "by_device_type": "Device Type"
        case "by_fleet": "Fleet"
        case "by_device": "Specific Device"
        default: targetType
        }
    }
}

struct RuleCondition: Codable, Sendable, Identifiable {
    let id: String
    let field: String
    let `operator`: String
    let value: String

    var operatorLabel: String {
        switch `operator` {
        case "gt": ">"
        case "gte": "≥"
        case "lt": "<"
        case "lte": "≤"
        case "eq": "="
        case "neq": "≠"
        default: `operator`
        }
    }

    var summary: String {
        "\(field) \(operatorLabel) \(value)"
    }
}

struct RuleAction: Codable, Sendable, Identifiable {
    let id: String
    let actionType: String
    let config: [String: AnyCodableValue]

    enum CodingKeys: String, CodingKey {
        case id, config
        case actionType = "action_type"
    }

    var actionLabel: String {
        switch actionType {
        case "create_alert": "Create Alert"
        case "send_webhook": "Send Webhook"
        case "send_command": "Send Command"
        default: actionType
        }
    }
}
