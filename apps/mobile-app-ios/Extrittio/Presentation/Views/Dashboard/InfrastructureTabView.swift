import SwiftUI

struct InfrastructureTabView: View {
    let metrics: CurrentMetricsResponse?
    let history: MetricsHistoryResponse?
    let metricsError: String?
    @Environment(\.horizontalSizeClass) private var sizeClass

    private var isCompact: Bool { sizeClass == .compact }
    private var iconSize: CGFloat { isCompact ? 26 : 32 }
    private var sparklineWidth: CGFloat { isCompact ? 50 : 70 }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                healthStrip
                    .slideIn(delay: 0)
                systemResourcesCard
                    .slideIn(delay: 0.07)
                appPerformanceCard
                    .slideIn(delay: 0.14)
            }
            .padding(.horizontal)
            .padding(.bottom, 20)
        }
    }

    // MARK: - Health Strip

    @ViewBuilder
    private var healthStrip: some View {
        let columns = isCompact
            ? [GridItem(.flexible()), GridItem(.flexible())]
            : [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())]

        LazyVGrid(columns: columns, spacing: 8) {
            HealthStripCard(
                label: "CPU",
                value: metrics.map { formatPercent($0.system.cpuUsagePercent) } ?? "—",
                progress: (metrics?.system.cpuUsagePercent ?? 0) / 100,
                color: .green
            )
            HealthStripCard(
                label: "Memory",
                value: metrics.map { formatPercent(memoryPercent($0.system)) } ?? "—",
                progress: (metrics.map { memoryPercent($0.system) } ?? 0) / 100,
                color: .blue
            )
            HealthStripCard(
                label: "Latency",
                value: metrics.map { formatMs($0.app.avgLatencyMs) } ?? "—",
                progress: metrics.map { min($0.app.avgLatencyMs / 500, 1) } ?? 0,
                color: .yellow
            )
            HealthStripCard(
                label: "Errors",
                value: metrics.map { "\($0.app.errorCount)" } ?? "—",
                progress: 0,
                color: metrics?.app.errorCount ?? 0 > 0 ? .red : .green
            )
        }
    }

    // MARK: - System Resources

    private var systemResourcesCard: some View {
        VStack(alignment: .leading, spacing: 14) {
            sectionHeader("System Resources")

            if let error = metricsError, metrics == nil {
                Text(error)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, 20)
            } else {
                MetricRow(
                    icon: "cpu",
                    iconColor: .green,
                    title: "CPU Usage",
                    sparklineData: cpuHistory,
                    value: metrics.map { formatPercent($0.system.cpuUsagePercent) } ?? "—",
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )

                MetricRow(
                    icon: "memorychip",
                    iconColor: .blue,
                    title: "Memory",
                    subtitle: metrics.map { formatBytes($0.system.memoryUsedBytes) + " / " + formatBytes($0.system.memoryTotalBytes) },
                    sparklineData: memoryHistory,
                    value: metrics.map { formatPercent(memoryPercent($0.system)) } ?? "—",
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )

                MetricRow(
                    icon: "internaldrive",
                    iconColor: .purple,
                    title: "Disk",
                    subtitle: metrics.map { formatBytes($0.system.diskUsedBytes) + " / " + formatBytes($0.system.diskTotalBytes) },
                    progress: metrics.map { Double($0.system.diskUsedBytes) / Double($0.system.diskTotalBytes) },
                    value: metrics.map { formatPercent(Double($0.system.diskUsedBytes) / Double($0.system.diskTotalBytes) * 100) } ?? "—",
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )

                MetricRow(
                    icon: "network",
                    iconColor: .cyan,
                    title: "Network I/O",
                    subtitle: metrics.map { "↓ \(formatBytesRate($0.system.networkRxBytesDelta))  ↑ \(formatBytesRate($0.system.networkTxBytesDelta))" },
                    sparklineData: networkRxHistory,
                    value: "",
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )

                if let m = metrics, m.system.loadAvg1m != nil {
                    loadAverageRow(m.system)
                }
            }
        }
        .padding(16)
        .background(.thinMaterial, in: .rect(cornerRadius: 16))
    }

    // MARK: - App Performance

    private var appPerformanceCard: some View {
        VStack(alignment: .leading, spacing: 14) {
            sectionHeader("Application Performance")

            if let error = metricsError, metrics == nil {
                Text(error)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, 20)
            } else {
                MetricRow(
                    icon: "arrow.up.arrow.down",
                    iconColor: .blue,
                    title: "Request Rate",
                    sparklineData: requestRateHistory,
                    value: metrics.map { "\($0.app.requestCount)/s" } ?? "—",
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )

                MetricRow(
                    icon: "exclamationmark.triangle",
                    iconColor: metrics?.app.errorCount ?? 0 > 0 ? .red : .green,
                    title: "Errors",
                    sparklineData: errorHistory,
                    value: metrics.map { "\($0.app.errorCount)" } ?? "—",
                    valueColor: metrics?.app.errorCount ?? 0 > 0 ? .red : .green,
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )

                MetricRow(
                    icon: "clock",
                    iconColor: .yellow,
                    title: "Latency",
                    subtitle: "avg / p95",
                    sparklineData: latencyHistory,
                    value: metrics.map { formatMs($0.app.avgLatencyMs) + " / " + formatMs($0.app.p95LatencyMs) } ?? "—",
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )

                MetricRow(
                    icon: "cylinder.split.1x2",
                    iconColor: .green,
                    title: "DB Pool",
                    subtitle: metrics.map { "\($0.app.dbPoolActive) active / \($0.app.dbPoolIdle) idle" },
                    progress: metrics.map { Double($0.app.dbPoolActive) / Double(max($0.app.dbPoolActive + $0.app.dbPoolIdle, 1)) },
                    value: metrics.map { "\($0.app.dbPoolActive)/\($0.app.dbPoolActive + $0.app.dbPoolIdle)" } ?? "—",
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )

                MetricRow(
                    icon: "arrow.left.arrow.right",
                    iconColor: .purple,
                    title: "Zenoh Throughput",
                    sparklineData: zenohInHistory,
                    value: metrics.map { "↓\($0.app.zenohMessagesIn) ↑\($0.app.zenohMessagesOut)/s" } ?? "—",
                    iconSize: iconSize,
                    sparklineWidth: sparklineWidth
                )
            }
        }
        .padding(16)
        .background(.thinMaterial, in: .rect(cornerRadius: 16))
    }

    // MARK: - Helpers

    private func loadAverageRow(_ system: SystemMetrics) -> some View {
        HStack {
            Image(systemName: "gauge.medium")
                .font(.system(size: iconSize * 0.44))
                .foregroundStyle(.yellow)
                .frame(width: iconSize, height: iconSize)
                .background(Color.yellow.opacity(0.15), in: .rect(cornerRadius: iconSize * 0.25))

            Text("Load Average")
                .font(.system(size: 13, weight: .semibold))

            Spacer()

            HStack(spacing: 12) {
                loadAvgValue(system.loadAvg1m, label: "1m")
                loadAvgValue(system.loadAvg5m, label: "5m")
                loadAvgValue(system.loadAvg15m, label: "15m")
            }
        }
    }

    private func loadAvgValue(_ value: Double?, label: String) -> some View {
        VStack(spacing: 0) {
            Text(value.map { String(format: "%.2f", $0) } ?? "—")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(.yellow)
            Text(label)
                .font(.system(size: 9))
                .foregroundStyle(.tertiary)
        }
    }

    private func sectionHeader(_ title: String) -> some View {
        Text(title)
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(.secondary)
            .textCase(.uppercase)
            .tracking(1)
    }

    // MARK: - Sparkline Data Extraction

    private var cpuHistory: [Double] { history?.system.map(\.cpuUsagePercent) ?? [] }
    private var memoryHistory: [Double] { history?.system.map { memoryPercent($0) } ?? [] }
    private var networkRxHistory: [Double] { history?.system.map { Double($0.networkRxBytesDelta) } ?? [] }
    private var requestRateHistory: [Double] { history?.app.map { Double($0.requestCount) } ?? [] }
    private var errorHistory: [Double] { history?.app.map { Double($0.errorCount) } ?? [] }
    private var latencyHistory: [Double] { history?.app.map(\.avgLatencyMs) ?? [] }
    private var zenohInHistory: [Double] { history?.app.map { Double($0.zenohMessagesIn) } ?? [] }

    // MARK: - Formatting

    private func memoryPercent(_ s: SystemMetrics) -> Double {
        s.memoryTotalBytes > 0 ? Double(s.memoryUsedBytes) / Double(s.memoryTotalBytes) * 100 : 0
    }

    private func formatPercent(_ value: Double) -> String {
        String(format: "%.0f%%", value)
    }

    private func formatMs(_ value: Double) -> String {
        if value < 1 {
            return String(format: "%.1fms", value)
        }
        return String(format: "%.0fms", value)
    }

    private func formatBytes(_ bytes: Int64) -> String {
        let gb = Double(bytes) / 1_073_741_824
        if gb >= 1 {
            return String(format: "%.1f GB", gb)
        }
        let mb = Double(bytes) / 1_048_576
        return String(format: "%.0f MB", mb)
    }

    private func formatBytesRate(_ bytes: Int64) -> String {
        let perSecond = Double(bytes) / 30.0
        if perSecond >= 1_048_576 {
            return String(format: "%.1f MB/s", perSecond / 1_048_576)
        }
        return String(format: "%.0f KB/s", perSecond / 1024)
    }
}
