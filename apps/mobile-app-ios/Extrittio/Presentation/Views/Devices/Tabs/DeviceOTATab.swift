import SwiftUI

struct DeviceOTATab: View {
    let viewModel: DeviceDetailViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @Environment(AuthViewModel.self) private var authViewModel
    @State private var selectedFirmwareId: Int?
    @State private var showConfirmation = false
    @State private var deployingFirmwareId: Int?

    private var canDeployFirmware: Bool {
        authViewModel.currentUser?.hasPermission(.firmwareDeploy) == true
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                if viewModel.isLoadingOTA {
                    LoadingView("Loading OTA data...")
                } else {
                    AdaptiveHStack(spacing: 16) {
                        availableFirmwareSection
                        deploymentHistorySection
                    }
                }
            }
            .padding()
        }
        .refreshable { await viewModel.loadOTA(forceRefresh: true) }
        .task { await viewModel.loadOTA() }
    }

    @ViewBuilder
    private var availableFirmwareSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Available Firmware")
                .font(.headline)

            if !connectionMonitor.isOnline, canDeployFirmware {
                OfflineActionHint(message: "Firmware deployment is unavailable while offline.")
            }

            if viewModel.availableFirmware.isEmpty {
                Text("No firmware updates available for this device type.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(viewModel.availableFirmware) { fw in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("v\(fw.version)")
                                .font(.callout.bold())
                            if let desc = fw.description {
                                Text(desc)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        Spacer()
                        if canDeployFirmware {
                            let isDeploying = deployingFirmwareId == fw.id
                            Button {
                                selectedFirmwareId = fw.id
                                showConfirmation = true
                            } label: {
                                HStack(spacing: Spacing.xs) {
                                    if isDeploying {
                                        ProgressView()
                                            .controlSize(.small)
                                    }
                                    Text(isDeploying ? "Deploying" : "Deploy")
                                }
                            }
                            .buttonStyle(.borderedProminent)
                            .controlSize(.small)
                            .disabled(!connectionMonitor.isOnline || deployingFirmwareId != nil)
                            .pressEffect()
                        }
                    }
                    .padding(.vertical, 4)
                    Divider()
                }
            }
        }
        .padding()
        .glassCard()
        .confirmationDialog("Deploy Firmware?", isPresented: $showConfirmation) {
            if let fwId = selectedFirmwareId {
                Button("Deploy") {
                    HapticEngine.shared.warning()
                    deployingFirmwareId = fwId
                    Task {
                        let success = await viewModel.triggerOTA(firmwareUpdateId: fwId)
                        deployingFirmwareId = nil
                        if success {
                            toastManager.show(.success("OTA deployment triggered"))
                        } else {
                            toastManager.show(.error("Failed to trigger OTA"))
                        }
                    }
                }
            }
        } message: {
            Text("This will trigger an OTA firmware update on the device.")
        }
    }

    @ViewBuilder
    private var deploymentHistorySection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Deployment History")
                .font(.headline)

            if viewModel.deployments.isEmpty {
                Text("No deployments yet.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(Array(viewModel.deployments.enumerated()), id: \.element.id) { index, dep in
                    VStack(alignment: .leading, spacing: 0) {
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(dep.firmwareVersion ?? "v?")
                                    .font(.callout.bold())
                                Text(String.formattedTimestamp(dep.initiatedAt))
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                if let duration = deploymentDuration(dep) {
                                    Text("Duration: \(duration)")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            Spacer()
                            deploymentStatusBadge(dep.status)
                        }
                        if let err = dep.errorMessage {
                            Text(err)
                                .font(.caption)
                                .foregroundStyle(.red)
                                .padding(.top, 4)
                        }
                        Divider()
                            .padding(.top, 8)
                    }
                    .slideIn(delay: Double(index) * 0.07)
                }
            }
        }
        .padding()
        .glassCard()
    }

    private func deploymentStatusBadge(_ status: String) -> some View {
        Text(status.capitalized)
            .font(.caption.bold())
            .foregroundStyle(deploymentColor(status))
            .padding(.horizontal, 8)
            .padding(.vertical, 2)
            .glassCard()
    }

    private func deploymentColor(_ status: String) -> Color {
        switch status {
        case "success": .green
        case "failed": .red
        case "pending", "downloading", "verifying", "installing", "rebooting": .blue
        default: .gray
        }
    }

    private func deploymentDuration(_ dep: OtaDeployment) -> String? {
        guard let completedAt = dep.completedAt,
              let start = Date.fromISO8601(dep.initiatedAt),
              let end = Date.fromISO8601(completedAt) else { return nil }
        let seconds = Int(end.timeIntervalSince(start))
        if seconds < 60 { return "\(seconds)s" }
        if seconds < 3600 { return "\(seconds / 60)m \(seconds % 60)s" }
        return "\(seconds / 3600)h \(seconds % 3600 / 60)m"
    }
}
