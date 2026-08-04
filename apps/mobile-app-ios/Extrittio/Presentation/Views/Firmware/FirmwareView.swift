import SwiftUI

struct FirmwareView: View {
    let viewModel: FirmwareViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @Environment(ToastManager.self) private var toastManager
    @State private var deletingFirmwareId: Int?

    private var canManageFirmware: Bool {
        authViewModel.currentUser?.hasPermission(.firmwareManage) == true
    }

    var body: some View {
        VStack(spacing: 0) {
            filterBar
            firmwareList
        }
        .navigationTitle("Firmware")
        .refreshable { await viewModel.load() }
        .task { await viewModel.load() }
    }

    private var filterBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                Menu {
                    Button("All Types") {
                        viewModel.selectedDeviceTypeId = nil
                        Task { await viewModel.load() }
                    }
                    ForEach(viewModel.deviceTypes) { dt in
                        Button(dt.name) {
                            viewModel.selectedDeviceTypeId = dt.id
                            Task { await viewModel.load() }
                        }
                    }
                } label: {
                    Label(
                        viewModel.deviceTypes.first(where: { $0.id == viewModel.selectedDeviceTypeId })?.name ?? "Device Type",
                        systemImage: "cpu"
                    )
                    .font(.subheadline)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .glassCard()
                }

                Spacer()

                Text("\(viewModel.totalFirmware) versions")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding()
        }
    }

    private var firmwareList: some View {
        Group {
            switch viewModel.firmwareState {
            case .loading:
                skeletonList

            case .loaded(let firmware), .cached(let firmware, _):
                ScrollView {
                    LazyVStack(spacing: Spacing.md) {
                        if let cacheDate = viewModel.firmwareState.cacheDate {
                            StalenessLabel(date: cacheDate)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }

                        if !connectionMonitor.isOnline, canManageFirmware {
                            OfflineActionHint(message: "Firmware deletion is unavailable while offline.")
                        }

                        ForEach(Array(firmware.enumerated()), id: \.element.id) { index, fw in
                            firmwareRow(fw)
                                .slideIn(delay: Double(index) * 0.06)
                                .contextMenu {
                                    if canManageFirmware {
                                        Button(role: .destructive) {
                                            HapticEngine.shared.impact(.medium)
                                            deletingFirmwareId = fw.id
                                            Task {
                                                let success = await viewModel.deleteFirmware(id: fw.id)
                                                deletingFirmwareId = nil
                                                toastManager.show(success ? .success("Firmware deleted") : .error("Failed to delete firmware"))
                                            }
                                        } label: {
                                            Label(deletingFirmwareId == fw.id ? "Deleting" : "Delete", systemImage: deletingFirmwareId == fw.id ? "hourglass" : "trash")
                                        }
                                        .disabled(deletingFirmwareId != nil || !connectionMonitor.isOnline)
                                    }
                                }
                                .opacity(deletingFirmwareId == fw.id ? 0.6 : 1)
                        }
                    }
                    .padding()
                }

            case .empty:
                EmptyStateView(
                    icon: "arrow.down.circle",
                    title: "No Firmware",
                    message: "No firmware updates found."
                )

            case .error(let error):
                ScrollView {
                    ErrorBanner(message: error.localizedDescription) {
                        HapticEngine.shared.warning()
                        Task { await viewModel.load() }
                    }
                    .padding()
                }

            case .loadingMore(let firmware):
                ScrollView {
                    LazyVStack(spacing: Spacing.md) {
                        ForEach(Array(firmware.enumerated()), id: \.element.id) { index, fw in
                            firmwareRow(fw)
                                .slideIn(delay: Double(index) * 0.06)
                        }
                        SkeletonView(shape: .rect(), height: 90)
                            .padding(.horizontal)
                    }
                    .padding()
                }
            }
        }
    }

    private var skeletonList: some View {
        ScrollView {
            LazyVStack(spacing: Spacing.md) {
                ForEach(0..<4, id: \.self) { _ in
                    firmwareSkeletonRow
                }
            }
            .padding()
        }
        .allowsHitTesting(false)
    }

    private var firmwareSkeletonRow: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                SkeletonView(shape: .rect(cornerRadius: 6), width: 60, height: 16)
                Spacer()
                SkeletonView(shape: .capsule, width: 48, height: 18)
            }

            HStack(spacing: 12) {
                SkeletonView(shape: .rect(cornerRadius: 4), width: 100, height: 12)
                SkeletonView(shape: .rect(cornerRadius: 4), width: 64, height: 12)
            }

            SkeletonView(shape: .rect(cornerRadius: 4), height: 12)
            SkeletonView(shape: .rect(cornerRadius: 4), width: 120, height: 10)
        }
        .padding()
        .glassCard()
    }

    private func firmwareRow(_ fw: FirmwareUpdate) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("v\(fw.version)")
                    .font(.body.bold())
                Spacer()
                if let source = fw.source {
                    Text(source.uppercased())
                        .font(.caption2.bold())
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .glassCard()
                }
            }

            HStack(spacing: 12) {
                if let typeName = fw.deviceTypeName {
                    Label(typeName, systemImage: "cpu")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if let size = fw.fileSize {
                    Label(formatBytes(size), systemImage: "doc")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if fw.hasBlob == true {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.caption)
                        .foregroundStyle(.green)
                }
            }

            if let desc = fw.description {
                Text(desc)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }

            Text(fw.createdAt)
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .padding()
        .glassCard()
    }

    private func formatBytes(_ bytes: Int) -> String {
        let kb = Double(bytes) / 1024
        if kb < 1024 { return String(format: "%.1f KB", kb) }
        let mb = kb / 1024
        return String(format: "%.1f MB", mb)
    }
}
