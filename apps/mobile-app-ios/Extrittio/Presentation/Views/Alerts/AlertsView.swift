import SwiftUI

private enum AlertListMutation {
    case acknowledge
    case resolve
}

struct AlertsView: View {
    let viewModel: AlertsViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @State private var workingAlertId: String?
    @State private var workingMutation: AlertListMutation?

    private let statuses = ["All", "active", "acknowledged", "resolved"]
    private let severities = ["All", "critical", "warning", "info"]

    private var canManageAlerts: Bool {
        authViewModel.currentUser?.hasPermission(.alertsManage) == true
    }

    private var hasActiveFilters: Bool {
        viewModel.statusFilter != nil || viewModel.severityFilter != nil
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                filterBar
                alertList
            }
            .navigationTitle("Alerts")
            .refreshable { await viewModel.load() }
            .task { await viewModel.load() }
        }
    }

    // MARK: - Filter Bar

    private var filterBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: Spacing.md) {
                Menu {
                    ForEach(statuses, id: \.self) { status in
                        Button(status.capitalized) {
                            HapticEngine.shared.selection()
                            viewModel.statusFilter = status == "All" ? nil : status
                            Task { await viewModel.load() }
                        }
                    }
                } label: {
                    Label(viewModel.statusFilter?.capitalized ?? "Status", systemImage: "line.3.horizontal.decrease.circle")
                        .font(.subheadline)
                        .padding(.horizontal, Spacing.md)
                        .padding(.vertical, Spacing.sm - 2)
                        .glassCard()
                }

                Menu {
                    ForEach(severities, id: \.self) { severity in
                        Button(severity.capitalized) {
                            HapticEngine.shared.selection()
                            viewModel.severityFilter = severity == "All" ? nil : severity
                            Task { await viewModel.load() }
                        }
                    }
                } label: {
                    Label(viewModel.severityFilter?.capitalized ?? "Severity", systemImage: "exclamationmark.triangle")
                        .font(.subheadline)
                        .padding(.horizontal, Spacing.md)
                        .padding(.vertical, Spacing.sm - 2)
                        .glassCard()
                }

                Spacer()

                Text("\(viewModel.totalAlerts) alerts")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(Spacing.lg)
        }
    }

    // MARK: - Alert List

    private var alertList: some View {
        Group {
            switch viewModel.alertsState {
            case .loading:
                AlertsSkeletonView()

            case .loaded(let alerts), .cached(let alerts, _), .loadingMore(let alerts):
                ScrollView {
                    LazyVStack(spacing: Spacing.md) {
                        if let cacheDate = viewModel.alertsState.cacheDate {
                            StalenessLabel(date: cacheDate)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }

                        if !connectionMonitor.isOnline, canManageAlerts, alerts.contains(where: { $0.isActive || $0.isAcknowledged }) {
                            OfflineActionHint(message: "Alert actions are unavailable while offline.")
                        }

                        ForEach(Array(alerts.enumerated()), id: \.element.id) { index, alert in
                            alertRow(alert)
                                .slideIn(delay: Double(index) * 0.06)
                        }
                    }
                    .padding(Spacing.lg)
                }

            case .error(let error):
                ScrollView {
                    ErrorBanner(message: error.localizedDescription) {
                        Task { await viewModel.load() }
                    }
                    .padding(Spacing.lg)
                }

            case .empty:
                ScrollView {
                    emptyAlertsView
                }
            }
        }
    }

    // MARK: - Alert Row

    private func alertRow(_ alert: Alert) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
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

            Text(alert.createdAt)
                .font(.caption2)
                .foregroundStyle(.secondary)

            if alert.isActive && canManageAlerts {
                HStack(spacing: Spacing.md) {
                    Button {
                        workingAlertId = alert.id
                        workingMutation = .acknowledge
                        Task {
                            let success = await viewModel.acknowledgeAlert(id: alert.id)
                            workingAlertId = nil
                            workingMutation = nil
                            if success {
                                toastManager.show(.success("Alert acknowledged"))
                            } else {
                                toastManager.show(.error("Failed to acknowledge alert"))
                            }
                        }
                    } label: {
                        ActionProgressLabel(
                            title: "Acknowledge",
                            systemImage: "checkmark.circle",
                            isLoading: isWorking(alert, .acknowledge),
                            loadingTitle: "Acknowledging"
                        )
                    }
                    .font(.caption.bold())
                    .buttonStyle(.bordered)
                    .disabled(!connectionMonitor.isOnline || workingAlertId != nil)

                    Button {
                        workingAlertId = alert.id
                        workingMutation = .resolve
                        Task {
                            let success = await viewModel.resolveAlert(id: alert.id)
                            workingAlertId = nil
                            workingMutation = nil
                            if success {
                                toastManager.show(.success("Alert resolved"))
                            } else {
                                toastManager.show(.error("Failed to resolve alert"))
                            }
                        }
                    } label: {
                        ActionProgressLabel(
                            title: "Resolve",
                            systemImage: "checkmark.seal",
                            isLoading: isWorking(alert, .resolve),
                            loadingTitle: "Resolving"
                        )
                    }
                    .font(.caption.bold())
                    .buttonStyle(.borderedProminent)
                    .disabled(!connectionMonitor.isOnline || workingAlertId != nil)
                }
            } else if alert.isAcknowledged && canManageAlerts {
                Button {
                    workingAlertId = alert.id
                    workingMutation = .resolve
                    Task {
                        let success = await viewModel.resolveAlert(id: alert.id)
                        workingAlertId = nil
                        workingMutation = nil
                        if success {
                            toastManager.show(.success("Alert resolved"))
                        } else {
                            toastManager.show(.error("Failed to resolve alert"))
                        }
                    }
                } label: {
                    ActionProgressLabel(
                        title: "Resolve",
                        systemImage: "checkmark.seal",
                        isLoading: isWorking(alert, .resolve),
                        loadingTitle: "Resolving"
                    )
                }
                .font(.caption.bold())
                .buttonStyle(.borderedProminent)
                .disabled(!connectionMonitor.isOnline || workingAlertId != nil)
            }
        }
        .padding(Spacing.lg)
        .glassCard()
        .opacity(alert.status == "resolved" ? 0.7 : 1)
        .overlay(alignment: .leading) {
            if alert.severity == "critical" {
                Rectangle()
                    .fill(Color.red.opacity(0.6))
                    .frame(width: 2)
                    .clipShape(.rect(cornerRadius: Spacing.md))
            }
        }
    }

    // MARK: - Helpers

    private func alertStatusColor(_ status: String) -> Color {
        switch status {
        case "active": .red
        case "acknowledged": .orange
        case "resolved": .green
        default: .gray
        }
    }

    private var emptyAlertsView: some View {
        EmptyStateView(
            icon: "bell.slash",
            title: "No Alerts",
            message: hasActiveFilters ? "No alerts match the selected filters." : "No alerts have been reported.",
            actionTitle: hasActiveFilters ? "Clear Filters" : nil,
            actionSystemImage: "xmark.circle"
        ) {
            clearFilters()
        }
    }

    private func clearFilters() {
        viewModel.statusFilter = nil
        viewModel.severityFilter = nil
        HapticEngine.shared.selection()
        Task { await viewModel.load() }
    }

    private func isWorking(_ alert: Alert, _ mutation: AlertListMutation) -> Bool {
        workingAlertId == alert.id && workingMutation == mutation
    }
}

// MARK: - Skeleton

private struct AlertsSkeletonView: View {
    private let rowWidths: [(CGFloat, CGFloat, CGFloat)] = [
        (80, 180, 130),
        (70, 210, 110),
        (90, 160, 150),
        (80, 195, 120)
    ]

    var body: some View {
        ScrollView {
            VStack(spacing: Spacing.md) {
                ForEach(0..<4, id: \.self) { index in
                    VStack(alignment: .leading, spacing: Spacing.sm) {
                        // Badge + status row
                        HStack {
                            SkeletonView(shape: .capsule, width: rowWidths[index].0, height: 20)
                            Spacer()
                            SkeletonView(shape: .rect(cornerRadius: 4), width: 60, height: 12)
                        }
                        // Message line
                        SkeletonView(shape: .rect(cornerRadius: 4), width: rowWidths[index].1, height: 14)
                        // Triggered / date
                        SkeletonView(shape: .rect(cornerRadius: 4), width: rowWidths[index].2, height: 11)
                        SkeletonView(shape: .rect(cornerRadius: 4), width: 100, height: 10)
                    }
                    .padding(Spacing.lg)
                    .glassCard()
                }
            }
            .padding(Spacing.lg)
        }
    }
}
