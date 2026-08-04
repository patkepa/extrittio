import SwiftUI

struct RulesView: View {
    let viewModel: RulesViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @Environment(ToastManager.self) private var toastManager
    @State private var togglingRuleId: String?
    @State private var deletingRuleId: String?

    private var canManageRules: Bool {
        authViewModel.currentUser?.hasPermission(.rulesManage) == true
    }

    var body: some View {
        NavigationStack {
            Group {
                switch viewModel.rulesState {
                case .loading:
                    rulesSkeletonView
                case .loaded(let rules), .cached(let rules, _):
                    rulesList(rules)
                case .error(let error):
                    ScrollView {
                        VStack(spacing: Spacing.md) {
                            if let msg = viewModel.errorMessage {
                                ErrorBanner(message: msg) {
                                    Task { await viewModel.load() }
                                }
                            } else {
                                ErrorBanner(message: error.localizedDescription) {
                                    Task { await viewModel.load() }
                                }
                            }
                        }
                        .padding()
                    }
                case .empty:
                    EmptyStateView(
                        icon: "gearshape.2",
                        title: "No Rules",
                        message: "No automation rules configured yet."
                    )
                case .loadingMore(let rules):
                    rulesList(rules)
                }
            }
            .navigationTitle("Rules")
            .refreshable { await viewModel.load() }
            .task { await viewModel.load() }
            .navigationDestination(for: String.self) { ruleId in
                if let rule = viewModel.rulesState.data?.first(where: { $0.id == ruleId }) {
                    RuleDetailView(rule: rule)
                }
            }
        }
    }

    // MARK: - Skeleton Loading

    private var rulesSkeletonView: some View {
        ScrollView {
            LazyVStack(spacing: Spacing.sm) {
                ForEach(0..<4, id: \.self) { _ in
                    skeletonRow
                }
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.md)
        }
    }

    private var skeletonRow: some View {
        HStack(spacing: Spacing.md) {
            SkeletonView(shape: .circle, width: 28, height: 28)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                SkeletonView(shape: .rect(cornerRadius: 4), width: 140, height: 14)
                HStack(spacing: Spacing.xs) {
                    SkeletonView(shape: .capsule, width: 80, height: 11)
                    SkeletonView(shape: .rect(cornerRadius: 2), width: 6, height: 11)
                    SkeletonView(shape: .capsule, width: 70, height: 11)
                }
            }

            Spacer()

            SkeletonView(shape: .capsule, width: 48, height: 28)
        }
        .padding(Spacing.md)
        .glassCard()
    }

    // MARK: - Rules List

    private func rulesList(_ rules: [Rule]) -> some View {
        ScrollView {
            LazyVStack(spacing: Spacing.sm) {
                if let errorMsg = viewModel.errorMessage {
                    ErrorBanner(message: errorMsg) {
                        Task { await viewModel.load() }
                    }
                    .slideIn(delay: 0)
                }

                if !connectionMonitor.isOnline, canManageRules {
                    OfflineActionHint(message: "Rule changes are unavailable while offline.")
                }

                ForEach(Array(rules.enumerated()), id: \.element.id) { index, rule in
                    NavigationLink(value: rule.id) {
                        ruleRow(rule)
                    }
                    .buttonStyle(.plain)
                    .pressEffect()
                    .slideIn(delay: Double(index) * 0.05)
                    .contextMenu {
                        if canManageRules {
                            Button(role: .destructive) {
                                deletingRuleId = rule.id
                                Task {
                                    let success = await viewModel.deleteRule(id: rule.id)
                                    deletingRuleId = nil
                                    toastManager.show(success ? .success("Rule deleted") : .error(viewModel.errorMessage ?? "Failed to delete rule"))
                                }
                            } label: {
                                Label(deletingRuleId == rule.id ? "Deleting" : "Delete", systemImage: deletingRuleId == rule.id ? "hourglass" : "trash")
                            }
                            .disabled(deletingRuleId != nil || !connectionMonitor.isOnline)
                        }
                    }
                    .opacity(deletingRuleId == rule.id ? 0.6 : 1)
                }
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.md)
        }
    }

    // MARK: - Rule Row

    private func ruleRow(_ rule: Rule) -> some View {
        HStack(spacing: Spacing.md) {
            Image(systemName: rule.enabled ? "bolt.circle.fill" : "bolt.circle")
                .font(.title2)
                .foregroundStyle(rule.enabled ? .green : .gray)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(rule.name)
                    .font(.body.bold())
                HStack(spacing: Spacing.sm) {
                    Text(rule.triggerLabel)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(rule.targetLabel)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if !rule.conditions.isEmpty {
                    Text(rule.conditions.map(\.summary).joined(separator: ", "))
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }

            Spacer()

            if canManageRules {
                HStack(spacing: Spacing.sm) {
                    if togglingRuleId == rule.id {
                        ProgressView()
                            .controlSize(.small)
                    }
                    Toggle("", isOn: Binding(
                        get: { rule.enabled },
                        set: { newValue in
                            HapticEngine.shared.selection()
                            togglingRuleId = rule.id
                            Task {
                                let success = await viewModel.toggleRule(id: rule.id, enabled: newValue)
                                togglingRuleId = nil
                                toastManager.show(success ? .success(newValue ? "Rule enabled" : "Rule disabled") : .error(viewModel.errorMessage ?? "Failed to update rule"))
                            }
                        }
                    ))
                    .labelsHidden()
                    .disabled(!connectionMonitor.isOnline || togglingRuleId != nil || deletingRuleId != nil)
                }
            }
        }
        .padding(Spacing.md)
        .glassCard()
        .opacity(rule.enabled ? 1.0 : 0.6)
    }
}
