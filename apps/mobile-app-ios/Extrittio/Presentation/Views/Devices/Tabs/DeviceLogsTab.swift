import SwiftUI

struct DeviceLogsTab: View {
    let viewModel: DeviceDetailViewModel
    private let levels = ["All", "DEBUG", "INFO", "WARN", "ERROR"]
    @State private var searchText = ""

    private var filteredLogs: [DeviceLog] {
        if searchText.isEmpty { return viewModel.logs }
        return viewModel.logs.filter { $0.message.localizedCaseInsensitiveContains(searchText) }
    }

    var body: some View {
        VStack(spacing: 0) {
            levelPicker
            TextField("Search logs...", text: $searchText)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .padding(10)
                .glassCard()
                .padding(.horizontal)
                .padding(.bottom, 8)
            logList
        }
        .refreshable { await viewModel.loadLogs(forceRefresh: true) }
        .task { await viewModel.loadLogs() }
    }

    private var levelPicker: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(levels, id: \.self) { level in
                    Button(level) {
                        HapticEngine.shared.selection()
                        viewModel.logLevelFilter = level == "All" ? nil : level
                        Task { await viewModel.resetLogsAndReload() }
                    }
                    .font(.caption.bold())
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .glassCard()
                }
            }
            .padding()
        }
    }

    private var logList: some View {
        ScrollView {
            if viewModel.isLoadingLogs && viewModel.logs.isEmpty {
                LoadingView("Loading logs...")
            } else if viewModel.logs.isEmpty {
                EmptyStateView(icon: "doc.text.magnifyingglass", title: "No Logs", message: "No log entries found.")
            } else {
                LazyVStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(filteredLogs.enumerated()), id: \.element.id) { index, log in
                        logRow(log)
                            .slideIn(delay: Double(index) * 0.07)
                    }
                }
                .padding()
            }
        }
    }

    private func logRow(_ log: DeviceLog) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Text(log.level)
                .font(.caption2.bold().monospaced())
                .foregroundStyle(logLevelColor(log.level))
                .frame(width: 44, alignment: .leading)

            VStack(alignment: .leading, spacing: 2) {
                Text(log.message)
                    .font(.caption.monospaced())
                Text(String.formattedTimestamp(log.createdAt))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }

            Spacer()
        }
        .padding(10)
        .glassCard()
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
