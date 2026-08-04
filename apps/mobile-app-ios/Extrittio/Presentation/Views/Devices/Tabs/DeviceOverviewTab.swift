import SwiftUI

struct DeviceOverviewTab: View {
    let viewModel: DeviceDetailViewModel
    @Environment(AuthViewModel.self) private var authViewModel

    var body: some View {
        ScrollView {
            if let device = viewModel.device {
                VStack(spacing: 16) {
                    fleetSection(device)
                    if canReadLogs {
                        recentLogsSection
                    }
                }
                .padding()
            }
        }
        .refreshable {
            if canReadLogs {
                await viewModel.loadLogs(forceRefresh: true)
            }
        }
        .task {
            if canReadLogs {
                await viewModel.loadLogs()
            }
        }
    }

    private var canReadLogs: Bool {
        authViewModel.currentUser?.hasPermission(.logsRead) == true
    }

    @ViewBuilder
    private func fleetSection(_ device: Device) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Fleet")
                .font(.headline)

            if device.fleetId == nil && device.fleetName == nil {
                HStack(spacing: 10) {
                    Image(systemName: "square.stack.3d.up.slash")
                        .foregroundStyle(.secondary)
                    Text("No fleet assigned")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    Spacer()
                }
                .padding(.vertical, 4)
                .slideIn(delay: 0.07)
            } else {
                DeviceInfoRow("Name", value: device.fleetName ?? "Unnamed")
                    .slideIn(delay: 0.07)
                if let fleetId = device.fleetId {
                    DeviceInfoRow("ID", value: "\(fleetId)")
                        .slideIn(delay: 0.14)
                }
            }
        }
        .padding()
        .glassCard()
        .slideIn(delay: 0.0)
    }

    @ViewBuilder
    private var recentLogsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Recent Logs")
                    .font(.headline)
                Spacer()
                if viewModel.isLoadingLogs && !viewModel.logs.isEmpty {
                    ProgressView()
                        .controlSize(.small)
                }
            }

            switch viewModel.logsState {
            case .loading where viewModel.logs.isEmpty:
                HStack(spacing: 10) {
                    ProgressView()
                        .controlSize(.small)
                    Text("Loading logs...")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    Spacer()
                }
                .padding(.vertical, 4)

            case .error(let error):
                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                    Text(error.localizedDescription)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Button("Retry") {
                        Task { await viewModel.loadLogs(forceRefresh: true) }
                    }
                    .font(.callout.bold())
                }
                .padding(.vertical, 4)

            default:
                if recentLogs.isEmpty {
                    HStack(spacing: 10) {
                        Image(systemName: "doc.text.magnifyingglass")
                            .foregroundStyle(.secondary)
                        Text("No recent logs.")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                        Spacer()
                    }
                    .padding(.vertical, 4)
                } else {
                    ForEach(Array(recentLogs.enumerated()), id: \.element.id) { index, log in
                        recentLogRow(log)
                            .slideIn(delay: Double(index) * 0.05)
                        if index < recentLogs.count - 1 {
                            Divider()
                        }
                    }
                }
            }
        }
        .padding()
        .glassCard()
        .slideIn(delay: 0.07)
    }

    private var recentLogs: [DeviceLog] {
        Array(viewModel.logs.prefix(4))
    }

    private func recentLogRow(_ log: DeviceLog) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Circle()
                .fill(logLevelColor(log.level))
                .frame(width: 8, height: 8)
                .padding(.top, 6)

            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Text(log.level)
                        .font(.caption2.bold().monospaced())
                        .foregroundStyle(logLevelColor(log.level))
                    Text(String.formattedTimestamp(log.createdAt))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }

                Text(log.message)
                    .font(.caption.monospaced())
                    .lineLimit(2)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(.vertical, 4)
    }

    private func logLevelColor(_ level: String) -> Color {
        switch level {
        case "ERROR": .red
        case "WARN": .orange
        case "INFO": .blue
        case "DEBUG": .gray
        default: .gray
        }
    }
}

struct DeviceDetailsTab: View {
    let viewModel: DeviceDetailViewModel
    @Environment(ToastManager.self) private var toastManager

    var body: some View {
        ScrollView {
            if let device = viewModel.device {
                VStack(spacing: 16) {
                    registrySection(device)
                }
                .padding()
            }
        }
    }

    @ViewBuilder
    private func registrySection(_ device: Device) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Registry")
                .font(.headline)

            HStack {
                Text("ID")
                    .foregroundStyle(.secondary)
                Spacer()
                Text(device.id)
                    .font(.callout.monospaced())
                    .lineLimit(1)
                    .truncationMode(.middle)
                Button {
                    HapticEngine.shared.impact(.light)
                    UIPasteboard.general.string = device.id
                    toastManager.show(.copied)
                } label: {
                    Image(systemName: "doc.on.doc")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .slideIn(delay: 0.07)

            DeviceInfoRow("Device Type ID", value: "\(device.deviceTypeId)")
                .slideIn(delay: 0.14)
        }
        .padding()
        .glassCard()
        .slideIn(delay: 0.0)
    }
}

private struct DeviceInfoRow: View {
    let label: String
    let value: String

    init(_ label: String, value: String) {
        self.label = label
        self.value = value
    }

    var body: some View {
        HStack {
            Text(label)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.callout.monospaced())
                .lineLimit(1)
                .truncationMode(.middle)
        }
    }
}
