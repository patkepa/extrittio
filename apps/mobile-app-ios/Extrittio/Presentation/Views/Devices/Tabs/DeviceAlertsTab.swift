import SwiftUI

private enum DeviceAlertMutation {
    case acknowledge
    case resolve
}

struct DeviceAlertsTab: View {
    let viewModel: DeviceDetailViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @State private var alertToAcknowledge: Alert?
    @State private var alertToResolve: Alert?
    @State private var workingAlertId: String?
    @State private var workingMutation: DeviceAlertMutation?

    private var canManageAlerts: Bool {
        authViewModel.currentUser?.hasPermission(.alertsManage) == true
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                if viewModel.isLoadingAlerts && viewModel.alerts.isEmpty {
                    LoadingView("Loading alerts...")
                } else if viewModel.alerts.isEmpty {
                    EmptyStateView(icon: "bell.slash", title: "No Alerts", message: "No alerts for this device.")
                } else {
                    ForEach(Array(viewModel.alerts.enumerated()), id: \.element.id) { index, alert in
                        alertRow(alert)
                            .slideIn(delay: Double(index) * 0.07)
                    }
                }
            }
            .padding()
        }
        .refreshable { await viewModel.loadAlerts(forceRefresh: true) }
        .task { await viewModel.loadAlerts() }
        .confirmationDialog("Acknowledge Alert?", isPresented: Binding(
            get: { alertToAcknowledge != nil },
            set: { if !$0 { alertToAcknowledge = nil } }
        )) {
            if canManageAlerts {
                Button("Acknowledge") {
                    if let alert = alertToAcknowledge {
                        let alertId = alert.id
                        workingAlertId = alertId
                        workingMutation = .acknowledge
                        Task {
                            let success = await viewModel.acknowledgeAlert(id: alertId)
                            workingAlertId = nil
                            workingMutation = nil
                            if success {
                                toastManager.show(.success("Alert acknowledged"))
                            } else {
                                toastManager.show(.error("Failed to acknowledge alert"))
                            }
                        }
                    }
                    alertToAcknowledge = nil
                }
            }
        } message: {
            Text("Mark this alert as acknowledged.")
        }
        .confirmationDialog("Resolve Alert?", isPresented: Binding(
            get: { alertToResolve != nil },
            set: { if !$0 { alertToResolve = nil } }
        )) {
            if canManageAlerts {
                Button("Resolve") {
                    if let alert = alertToResolve {
                        let alertId = alert.id
                        workingAlertId = alertId
                        workingMutation = .resolve
                        Task {
                            let success = await viewModel.resolveAlert(id: alertId)
                            workingAlertId = nil
                            workingMutation = nil
                            if success {
                                toastManager.show(.success("Alert resolved"))
                            } else {
                                toastManager.show(.error("Failed to resolve alert"))
                            }
                        }
                    }
                    alertToResolve = nil
                }
            }
        } message: {
            Text("Mark this alert as resolved.")
        }
    }

    private func alertRow(_ alert: Alert) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                SeverityBadge(severity: alert.severity)
                Spacer()
                Text(alert.status.capitalized)
                    .font(.caption.bold())
                    .foregroundStyle(alertStatusColor(alert.status))
            }

            Text(alert.message)
                .font(.callout)

            if let value = alert.triggeredValue {
                Text("Triggered: \(value)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Text(String.formattedTimestamp(alert.createdAt))
                .font(.caption2)
                .foregroundStyle(.secondary)

            if alert.isActive && canManageAlerts {
                HStack(spacing: 12) {
                    Button {
                        alertToAcknowledge = alert
                    } label: {
                        ActionProgressLabel(
                            title: "Acknowledge",
                            systemImage: "checkmark.circle",
                            isLoading: isWorking(alert, .acknowledge),
                            loadingTitle: "Acknowledging"
                        )
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.regular)
                    .disabled(!connectionMonitor.isOnline || workingAlertId != nil)
                    .pressEffect()

                    Button {
                        alertToResolve = alert
                    } label: {
                        ActionProgressLabel(
                            title: "Resolve",
                            systemImage: "checkmark.seal",
                            isLoading: isWorking(alert, .resolve),
                            loadingTitle: "Resolving"
                        )
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.regular)
                    .disabled(!connectionMonitor.isOnline || workingAlertId != nil)
                    .pressEffect()
                }
            } else if alert.isAcknowledged && canManageAlerts {
                Button {
                    alertToResolve = alert
                } label: {
                    ActionProgressLabel(
                        title: "Resolve",
                        systemImage: "checkmark.seal",
                        isLoading: isWorking(alert, .resolve),
                        loadingTitle: "Resolving"
                    )
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.regular)
                .disabled(!connectionMonitor.isOnline || workingAlertId != nil)
                .pressEffect()
            }
        }
        .padding()
        .glassCard()
        .opacity(alert.status == "resolved" ? 0.7 : 1)
        .overlay(alignment: .leading) {
            if alert.severity == "critical" {
                Rectangle()
                    .fill(Color.red.opacity(0.6))
                    .frame(width: 2)
            }
        }
    }

    private func isWorking(_ alert: Alert, _ mutation: DeviceAlertMutation) -> Bool {
        workingAlertId == alert.id && workingMutation == mutation
    }

    private func alertStatusColor(_ status: String) -> Color {
        switch status {
        case "active": .red
        case "acknowledged": .orange
        case "resolved": .green
        default: .gray
        }
    }
}
