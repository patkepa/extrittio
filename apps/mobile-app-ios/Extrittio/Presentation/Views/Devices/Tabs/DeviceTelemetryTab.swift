import SwiftUI
import Charts

struct DeviceTelemetryTab: View {
    @Bindable var viewModel: DeviceDetailViewModel
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var visibleTempCount = 0
    @State private var visibleHumCount = 0
    @State private var visibleBatCount = 0

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                // Time range picker
                Picker("Time Range", selection: $viewModel.telemetryTimeRange) {
                    ForEach(TelemetryTimeRange.allCases, id: \.self) { range in
                        Text(range.label).tag(range)
                    }
                }
                .pickerStyle(.segmented)

                if viewModel.isLoadingTelemetry && viewModel.telemetry.isEmpty {
                    VStack(spacing: 16) {
                        ChartSkeletonView()
                        ChartSkeletonView()
                        ChartSkeletonView()
                    }
                } else if viewModel.telemetry.isEmpty {
                    EmptyStateView(icon: "chart.line.uptrend.xyaxis", title: "No Telemetry", message: "No telemetry data available yet.")
                } else {
                    if sizeClass == .regular {
                        HStack(alignment: .top, spacing: 16) {
                            temperatureChart
                            humidityChart
                        }
                    } else {
                        temperatureChart
                        humidityChart
                    }
                    batteryChart
                    telemetryTable
                }
            }
            .padding()
        }
        .refreshable { await viewModel.loadTelemetry(forceRefresh: true) }
        .task { await viewModel.loadTelemetry() }
        .onChange(of: viewModel.telemetryTimeRange) { _, _ in
            HapticEngine.shared.selection()
            resetVisibleCounts()
            Task { await viewModel.resetTelemetryAndReload() }
        }
        .onChange(of: viewModel.telemetry.count) { _, _ in
            animateCharts(dataCount: viewModel.telemetry.count)
        }
    }

    private func resetVisibleCounts() {
        visibleTempCount = 0
        visibleHumCount = 0
        visibleBatCount = 0
    }

    private func animateCharts(dataCount: Int) {
        let tempData = parsedData(\.temperature)
        let humData = parsedData(\.humidity)
        let batData = parsedData(\.batteryLevel)

        withAnimation(AppAnimation.chart.animation) {
            visibleTempCount = tempData.count
            visibleHumCount = humData.count
            visibleBatCount = batData.count
        }
    }

    @ViewBuilder
    private var temperatureChart: some View {
        let data = parsedData(\.temperature)
        if !data.isEmpty {
            let visible = Array(data.prefix(max(visibleTempCount, 0)))
            chartSection(title: "Temperature", unit: "°C", data: visible, color: .orange)
                .onAppear {
                    withAnimation(AppAnimation.chart.animation) {
                        visibleTempCount = data.count
                    }
                }
        }
    }

    @ViewBuilder
    private var humidityChart: some View {
        let data = parsedData(\.humidity)
        if !data.isEmpty {
            let visible = Array(data.prefix(max(visibleHumCount, 0)))
            chartSection(title: "Humidity", unit: "%", data: visible, color: .blue)
                .onAppear {
                    withAnimation(AppAnimation.chart.animation) {
                        visibleHumCount = data.count
                    }
                }
        }
    }

    @ViewBuilder
    private var batteryChart: some View {
        let data = parsedData(\.batteryLevel)
        if !data.isEmpty {
            let visible = Array(data.prefix(max(visibleBatCount, 0)))
            chartSection(title: "Battery Level", unit: "%", data: visible, color: .green)
                .onAppear {
                    withAnimation(AppAnimation.chart.animation) {
                        visibleBatCount = data.count
                    }
                }
        }
    }

    private func parsedData(_ keyPath: KeyPath<TelemetryRecord, Double?>) -> [(Date, Double)] {
        viewModel.telemetry.compactMap { r -> (Date, Double)? in
            guard let value = r[keyPath: keyPath],
                  let date = Date.fromISO8601(r.receivedAt) else { return nil }
            return (date, value)
        }
        .sorted { $0.0 < $1.0 }
    }

    private func chartSection(title: String, unit: String, data: [(Date, Double)], color: Color) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.headline)

            Chart {
                ForEach(data, id: \.0) { date, value in
                    LineMark(
                        x: .value("Time", date),
                        y: .value(title, value)
                    )
                    .foregroundStyle(color)
                    .interpolationMethod(.catmullRom)

                    AreaMark(
                        x: .value("Time", date),
                        y: .value(title, value)
                    )
                    .interpolationMethod(.catmullRom)
                    .foregroundStyle(
                        LinearGradient(
                            colors: [color.opacity(0.2), color.opacity(0.0)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                    )
                }
            }
            .frame(height: 180)
            .chartYAxisLabel(unit)
            .chartXAxis {
                AxisMarks(values: .automatic(desiredCount: 5)) { value in
                    AxisValueLabel(format: .dateTime.hour().minute())
                    AxisGridLine()
                }
            }
        }
        .padding()
        .glassCard()
    }

    @ViewBuilder
    private var telemetryTable: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Recent Readings")
                .font(.headline)

            ForEach(Array(viewModel.telemetry.prefix(20).enumerated()), id: \.element.id) { index, record in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        HStack(spacing: 12) {
                            if let t = record.temperature {
                                Label("\(t, specifier: "%.1f")°C", systemImage: "thermometer")
                                    .font(.caption)
                            }
                            if let h = record.humidity {
                                Label("\(h, specifier: "%.1f")%", systemImage: "humidity")
                                    .font(.caption)
                            }
                            if let b = record.batteryLevel {
                                Label("\(b, specifier: "%.0f")%", systemImage: "battery.100")
                                    .font(.caption)
                            }
                        }
                    }
                    Spacer()
                    Text(String.formattedTimestamp(record.receivedAt))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)
                .slideIn(delay: Double(index) * 0.04)
                Divider()
            }
        }
        .padding()
        .glassCard()
    }
}
