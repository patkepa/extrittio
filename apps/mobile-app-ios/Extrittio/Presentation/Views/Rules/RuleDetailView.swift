import SwiftUI

struct RuleDetailView: View {
    let rule: Rule

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                infoSection
                conditionsSection
                actionsSection
            }
            .padding()
        }
        .navigationTitle(rule.name)
        .navigationBarTitleDisplayMode(.inline)
    }

    private var infoSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Rule Information")
                .font(.headline)

            infoRow("Status", value: rule.enabled ? "Enabled" : "Disabled")
            infoRow("Trigger", value: rule.triggerLabel)
            infoRow("Target", value: rule.targetLabel)
            if let targetId = rule.targetId {
                infoRow("Target ID", value: targetId)
            }
            infoRow("Cooldown", value: "\(rule.cooldownSeconds)s")
            if let desc = rule.description {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Description")
                        .foregroundStyle(.secondary)
                    Text(desc)
                        .font(.callout)
                }
            }
            infoRow("Created", value: rule.createdAt)
            infoRow("Updated", value: rule.updatedAt)
        }
        .padding()
        .glassCard()
    }

    private var conditionsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Conditions")
                .font(.headline)

            if rule.conditions.isEmpty {
                Text("No conditions defined.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(rule.conditions) { condition in
                    HStack {
                        Image(systemName: "arrow.right.circle")
                            .foregroundStyle(.blue)
                        Text(condition.field)
                            .font(.callout.bold())
                        Text(condition.operatorLabel)
                            .font(.callout)
                            .foregroundStyle(.orange)
                        Text(condition.value)
                            .font(.callout.monospaced())
                    }
                    .padding(.vertical, 2)
                }
            }
        }
        .padding()
        .glassCard()
    }

    private var actionsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Actions")
                .font(.headline)

            if rule.actions.isEmpty {
                Text("No actions defined.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(rule.actions) { action in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Image(systemName: actionIcon(action.actionType))
                                .foregroundStyle(.purple)
                            Text(action.actionLabel)
                                .font(.callout.bold())
                        }
                        if !action.config.isEmpty {
                            Text(action.config.map { "\($0.key): \($0.value.displayString)" }.joined(separator: ", "))
                                .font(.caption.monospaced())
                                .foregroundStyle(.secondary)
                        }
                    }
                    .padding(.vertical, 2)
                }
            }
        }
        .padding()
        .glassCard()
    }

    private func infoRow(_ label: String, value: String) -> some View {
        HStack {
            Text(label)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.callout)
        }
    }

    private func actionIcon(_ type: String) -> String {
        switch type {
        case "create_alert": "bell.badge"
        case "send_webhook": "network"
        case "send_command": "terminal"
        default: "gear"
        }
    }
}
