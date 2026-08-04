import SwiftUI

struct DevicesTabView: View {
    let stats: DashboardStats?
    let alertSummary: AlertSummary?
    var openDevices: (() -> Void)?
    var openAlerts: (() -> Void)?
    @Environment(\.horizontalSizeClass) private var sizeClass

    private var isCompact: Bool { sizeClass == .compact }
    private var totalDevices: Int { stats?.totalDevices ?? 0 }
    private var activeDevices: Int { stats?.activeDevices ?? 0 }
    private var offlineDevices: Int { stats?.offlineDevices ?? 0 }
    private var otherDevices: Int { max(totalDevices - activeDevices - offlineDevices, 0) }
    private var totalActiveAlerts: Int { alertSummary?.totalActive ?? 0 }
    private var criticalAlerts: Int { alertSummary?.active.critical ?? 0 }
    private var warningAlerts: Int { alertSummary?.active.warning ?? 0 }

    private var availabilityScore: Double? {
        guard let stats, stats.totalDevices > 0 else { return nil }
        return Double(stats.activeDevices) / Double(stats.totalDevices)
    }

    private var fleetHealthScore: Double? {
        guard let availabilityScore else { return nil }
        let deviceCount = Double(max(totalDevices, 1))
        let alertPenalty = min(Double(totalActiveAlerts) / deviceCount * 0.15, 0.15)
        let criticalPenalty = min(Double(criticalAlerts) * 0.18, 0.3)
        return max(0, availabilityScore - alertPenalty - criticalPenalty)
    }

    private var healthTone: AttentionTone {
        guard let score = fleetHealthScore else { return .info }
        if criticalAlerts > 0 || score < 0.7 { return .critical }
        if totalActiveAlerts > 0 || offlineDevices > 0 || score < 0.9 { return .warning }
        return .healthy
    }

    private var healthLabel: String {
        guard stats?.totalDevices ?? 0 > 0 else { return stats == nil ? "No Data" : "No Devices" }
        switch healthTone {
        case .critical: return "Critical"
        case .warning: return "Watch"
        case .healthy: return "Healthy"
        case .info: return "Unknown"
        }
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                fleetHealthSection
                    .slideIn(delay: 0)
                attentionQueueSection
                    .slideIn(delay: 0.06)
                quickActionsSection
                    .slideIn(delay: 0.1)
                statusOverviewSection
                    .slideIn(delay: 0.14)
                statCardsSection
                    .slideIn(delay: 0.18)
                alertSummarySection
                    .slideIn(delay: 0.22)
            }
            .padding(.horizontal)
            .padding(.bottom, 20)
        }
    }

    // MARK: - Fleet Health

    private var fleetHealthSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Fleet Health")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(.secondary)
                        .textCase(.uppercase)
                        .tracking(0.8)

                    HStack(spacing: 8) {
                        Image(systemName: healthTone.symbol)
                            .font(.system(size: 15, weight: .semibold))
                        Text(healthLabel)
                            .font(.system(size: 24, weight: .heavy))
                    }
                    .foregroundStyle(healthTone.color)
                }

                Spacer(minLength: 12)

                VStack(alignment: .trailing, spacing: 0) {
                    Text(fleetHealthScore.map(formatScore) ?? "—")
                        .font(.system(size: 40, weight: .heavy))
                        .foregroundStyle(healthTone.color)
                        .contentTransition(.numericText(value: fleetHealthScore ?? 0))
                    Text("health")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundStyle(.secondary)
                        .textCase(.uppercase)
                        .tracking(0.6)
                }
            }

            ProgressView(value: fleetHealthScore ?? 0, total: 1)
                .tint(healthTone.color)
                .scaleEffect(x: 1, y: 1.25, anchor: .center)

            LazyVGrid(columns: healthMetricColumns, spacing: 10) {
                healthMetric(
                    title: "Online",
                    value: stats.map { "\($0.activeDevices)" } ?? "—",
                    caption: availabilityScore.map { formatPercent($0 * 100) } ?? "availability",
                    color: .green,
                    systemImage: "checkmark.circle.fill"
                )
                healthMetric(
                    title: "Offline",
                    value: stats.map { "\($0.offlineDevices)" } ?? "—",
                    caption: offlineDevices == 1 ? "device" : "devices",
                    color: offlineDevices > 0 ? .red : .gray,
                    systemImage: offlineDevices > 0 ? "wifi.slash" : "moon.zzz"
                )
                healthMetric(
                    title: "Alerts",
                    value: alertSummary.map { "\($0.totalActive)" } ?? "—",
                    caption: criticalAlerts > 0 ? "\(criticalAlerts) critical" : "active",
                    color: totalActiveAlerts > 0 ? .orange : .green,
                    systemImage: totalActiveAlerts > 0 ? "bell.badge.fill" : "bell"
                )
            }
        }
        .padding(18)
        .background(healthTone.color.opacity(0.08), in: .rect(cornerRadius: 18))
        .background(.thinMaterial, in: .rect(cornerRadius: 18))
        .overlay {
            RoundedRectangle(cornerRadius: 18)
                .stroke(healthTone.color.opacity(0.16), lineWidth: 1)
        }
    }

    private var healthMetricColumns: [GridItem] {
        [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())]
    }

    private func healthMetric(title: String, value: String, caption: String, color: Color, systemImage: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: systemImage)
                .font(.system(size: isCompact ? 12 : 13, weight: .semibold))
                .foregroundStyle(color)
                .frame(width: 24, height: 24)
                .background(color.opacity(0.14), in: .rect(cornerRadius: 7))

            VStack(alignment: .leading, spacing: 1) {
                Text(value)
                    .font(.system(size: isCompact ? 17 : 19, weight: .heavy))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                Text(title)
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .textCase(.uppercase)
                    .lineLimit(1)
                Text(caption)
                    .font(.system(size: 9))
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(Color.primary.opacity(0.035), in: .rect(cornerRadius: 10))
    }

    // MARK: - Attention Queue

    private var attentionQueueSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            sectionHeader("Needs Attention")

            if attentionItems.isEmpty {
                attentionRow(
                    AttentionItem(
                        id: "clear",
                        title: "No active device issues",
                        detail: "Fleet is online and alerts are clear",
                        tone: .healthy,
                        action: nil
                    )
                )
            } else {
                ForEach(attentionItems) { item in
                    attentionRow(item)
                }
            }
        }
        .padding(16)
        .background(.thinMaterial, in: .rect(cornerRadius: 16))
    }

    private var attentionItems: [AttentionItem] {
        if stats == nil {
            var items = [
                AttentionItem(
                    id: "missing-data",
                    title: "Device status unavailable",
                    detail: "Refresh or check the server connection",
                    tone: .info,
                    action: nil
                )
            ]

            if criticalAlerts > 0 {
                items.append(
                    AttentionItem(
                        id: "critical-alerts",
                        title: "\(criticalAlerts) critical \(criticalAlerts == 1 ? "alert" : "alerts")",
                        detail: "Open alert triage",
                        tone: .critical,
                        action: openAlerts
                    )
                )
            }

            if warningAlerts > 0 {
                items.append(
                    AttentionItem(
                        id: "warning-alerts",
                        title: "\(warningAlerts) warning \(warningAlerts == 1 ? "alert" : "alerts")",
                        detail: "Review warning conditions",
                        tone: .warning,
                        action: openAlerts
                    )
                )
            }

            return Array(items.prefix(3))
        }

        if totalDevices == 0 {
            return [
                AttentionItem(
                    id: "no-devices",
                    title: "No registered devices",
                    detail: "Open Devices to add or inspect inventory",
                    tone: .info,
                    action: openDevices
                )
            ]
        }

        var items: [AttentionItem] = []

        if criticalAlerts > 0 {
            items.append(
                AttentionItem(
                    id: "critical-alerts",
                    title: "\(criticalAlerts) critical \(criticalAlerts == 1 ? "alert" : "alerts")",
                    detail: "Open alert triage",
                    tone: .critical,
                    action: openAlerts
                )
            )
        }

        if warningAlerts > 0 {
            items.append(
                AttentionItem(
                    id: "warning-alerts",
                    title: "\(warningAlerts) warning \(warningAlerts == 1 ? "alert" : "alerts")",
                    detail: "Review warning conditions",
                    tone: .warning,
                    action: openAlerts
                )
            )
        }

        if offlineDevices > 0 {
            items.append(
                AttentionItem(
                    id: "offline-devices",
                    title: "\(offlineDevices) offline \(offlineDevices == 1 ? "device" : "devices")",
                    detail: availabilityScore.map { "\(formatPercent($0 * 100)) fleet availability" } ?? "Open device inventory",
                    tone: offlineDevices > max(totalDevices / 5, 1) ? .critical : .warning,
                    action: openDevices
                )
            )
        }

        if let availabilityScore, availabilityScore < 0.9, offlineDevices == 0 {
            items.append(
                AttentionItem(
                    id: "low-availability",
                    title: "Availability below target",
                    detail: "\(formatPercent(availabilityScore * 100)) of devices are online",
                    tone: .warning,
                    action: openDevices
                )
            )
        }

        return Array(items.prefix(3))
    }

    @ViewBuilder
    private func attentionRow(_ item: AttentionItem) -> some View {
        if let action = item.action {
            Button {
                HapticEngine.shared.selection()
                action()
            } label: {
                attentionRowContent(item, showsChevron: true)
            }
            .buttonStyle(.plain)
        } else {
            attentionRowContent(item, showsChevron: false)
        }
    }

    private func attentionRowContent(_ item: AttentionItem, showsChevron: Bool) -> some View {
        HStack(spacing: 12) {
            Image(systemName: item.tone.symbol)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(item.tone.color)
                .frame(width: 30, height: 30)
                .background(item.tone.color.opacity(0.13), in: .rect(cornerRadius: 8))

            VStack(alignment: .leading, spacing: 2) {
                Text(item.title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                Text(item.detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer(minLength: 10)

            if showsChevron {
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(.tertiary)
            }
        }
        .padding(10)
        .background(item.tone.color.opacity(0.06), in: .rect(cornerRadius: 12))
    }

    // MARK: - Quick Actions

    @ViewBuilder
    private var quickActionsSection: some View {
        if openDevices != nil || openAlerts != nil {
            HStack(spacing: 10) {
                quickActionButton(
                    title: "Open Devices",
                    systemImage: "sensor.tag.radiowaves.forward",
                    tint: .blue,
                    action: openDevices
                )
                quickActionButton(
                    title: "Open Alerts",
                    systemImage: "bell.badge",
                    tint: totalActiveAlerts > 0 ? .red : .orange,
                    action: openAlerts
                )
            }
        }
    }

    @ViewBuilder
    private func quickActionButton(title: String, systemImage: String, tint: Color, action: (() -> Void)?) -> some View {
        if let action {
            Button {
                HapticEngine.shared.selection()
                action()
            } label: {
                Label(title, systemImage: systemImage)
                    .font(.system(size: 13, weight: .semibold))
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
            .tint(tint)
            .pressEffect()
        }
    }

    // MARK: - Status Overview

    private var statusOverviewSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            sectionHeader("Status Mix")

            if isCompact {
                VStack(spacing: 16) {
                    donutChart
                    statusBars
                }
            } else {
                HStack(alignment: .center, spacing: 20) {
                    donutChart
                    statusBars
                }
            }
        }
        .padding(16)
        .background(.thinMaterial, in: .rect(cornerRadius: 16))
    }

    private var donutChart: some View {
        DonutChartView(
            online: activeDevices,
            offline: offlineDevices,
            diameter: isCompact ? 92 : 118
        )
        .frame(maxWidth: .infinity)
    }

    private var statusBars: some View {
        VStack(spacing: 12) {
            statusBar(label: "Online", count: activeDevices, total: totalDevices, color: .green)
            statusBar(label: "Offline", count: offlineDevices, total: totalDevices, color: .red)
            if otherDevices > 0 {
                statusBar(label: "Other", count: otherDevices, total: totalDevices, color: .yellow)
            }
        }
        .frame(maxWidth: .infinity)
    }

    private func statusBar(label: String, count: Int, total: Int, color: Color) -> some View {
        let progress = total > 0 ? Double(count) / Double(total) : 0

        return VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(label)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.primary)
                Spacer()
                Text("\(count)")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(color)
                Text(total > 0 ? formatPercent(progress * 100) : "—")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule()
                        .fill(Color.primary.opacity(0.07))
                    Capsule()
                        .fill(color)
                        .frame(width: progress > 0 ? max(CGFloat(progress) * proxy.size.width, 4) : 0)
                }
            }
            .frame(height: 8)
        }
    }

    // MARK: - Stat Cards

    @ViewBuilder
    private var statCardsSection: some View {
        if isCompact {
            compactStatCards
        } else {
            regularStatCards
        }
    }

    private var regularStatCards: some View {
        VStack(spacing: 10) {
            HStack(spacing: 10) {
                StatCard(
                    icon: "sensor.tag.radiowaves.forward",
                    iconColor: .blue,
                    label: "Total",
                    value: stats.map { "\($0.totalDevices)" } ?? "—",
                    descriptor: "devices"
                )
                StatCard(
                    icon: "checkmark.circle.fill",
                    iconColor: .green,
                    label: "Active",
                    value: stats.map { "\($0.activeDevices)" } ?? "—",
                    descriptor: "online"
                )
                StatCard(
                    icon: "xmark.circle.fill",
                    iconColor: .red,
                    label: "Offline",
                    value: stats.map { "\($0.offlineDevices)" } ?? "—",
                    descriptor: "devices"
                )
            }
            HStack(spacing: 10) {
                StatCard(
                    icon: "envelope.fill",
                    iconColor: .purple,
                    label: "Messages",
                    value: stats.map { "\($0.totalMessages)" } ?? "—",
                    descriptor: "total"
                )
                StatCard(
                    icon: "bell.badge.fill",
                    iconColor: .red,
                    label: "Alerts",
                    value: alertSummary.map { "\($0.totalActive)" } ?? "—",
                    descriptor: "active",
                    highlighted: totalActiveAlerts > 0
                )
            }
        }
    }

    private var compactStatCards: some View {
        VStack(spacing: 8) {
            HStack(spacing: 8) {
                StatCard(
                    icon: "sensor.tag.radiowaves.forward",
                    iconColor: .blue,
                    label: "Total",
                    value: stats.map { "\($0.totalDevices)" } ?? "—",
                    descriptor: "devices"
                )
                StatCard(
                    icon: "checkmark.circle.fill",
                    iconColor: .green,
                    label: "Active",
                    value: stats.map { "\($0.activeDevices)" } ?? "—",
                    descriptor: "online"
                )
            }
            HStack(spacing: 8) {
                StatCard(
                    icon: "xmark.circle.fill",
                    iconColor: .red,
                    label: "Offline",
                    value: stats.map { "\($0.offlineDevices)" } ?? "—",
                    descriptor: "devices"
                )
                StatCard(
                    icon: "envelope.fill",
                    iconColor: .purple,
                    label: "Messages",
                    value: stats.map { "\($0.totalMessages)" } ?? "—",
                    descriptor: "total"
                )
            }
            StatCard(
                icon: "bell.badge.fill",
                iconColor: .red,
                label: "Active Alerts",
                value: alertSummary.map { "\($0.totalActive)" } ?? "—",
                descriptor: "active",
                highlighted: totalActiveAlerts > 0
            )
        }
    }

    // MARK: - Alert Summary

    @ViewBuilder
    private var alertSummarySection: some View {
        if let summary = alertSummary, summary.totalActive > 0 {
            VStack(alignment: .leading, spacing: 8) {
                sectionHeader("Alert Summary")

                HStack(spacing: 10) {
                    alertSeverityCard(count: summary.active.critical, label: "Critical", color: .red)
                    alertSeverityCard(count: summary.active.warning, label: "Warning", color: .orange)
                    alertSeverityCard(count: summary.active.info, label: "Info", color: .blue)
                }
            }
        }
    }

    private func alertSeverityCard(count: Int, label: String, color: Color) -> some View {
        VStack(spacing: 2) {
            Text("\(count)")
                .font(.system(size: 22, weight: .heavy))
                .foregroundStyle(color)
            Text(label)
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(color)
                .textCase(.uppercase)
                .tracking(0.5)
        }
        .frame(maxWidth: .infinity)
        .padding(12)
        .background(color.opacity(0.1), in: .rect(cornerRadius: 10))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(color.opacity(0.2), lineWidth: 1)
        )
    }

    // MARK: - Helpers

    private func sectionHeader(_ title: String) -> some View {
        Text(title)
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(.secondary)
            .textCase(.uppercase)
            .tracking(1)
    }

    private func formatScore(_ score: Double) -> String {
        "\(Int((score * 100).rounded()))%"
    }

    private func formatPercent(_ value: Double) -> String {
        String(format: "%.0f%%", value)
    }
}

private struct AttentionItem: Identifiable {
    let id: String
    let title: String
    let detail: String
    let tone: AttentionTone
    let action: (() -> Void)?
}

private enum AttentionTone {
    case critical
    case warning
    case healthy
    case info

    var color: Color {
        switch self {
        case .critical: .red
        case .warning: .orange
        case .healthy: .green
        case .info: .blue
        }
    }

    var symbol: String {
        switch self {
        case .critical: "exclamationmark.octagon.fill"
        case .warning: "exclamationmark.triangle.fill"
        case .healthy: "checkmark.seal.fill"
        case .info: "info.circle.fill"
        }
    }
}
