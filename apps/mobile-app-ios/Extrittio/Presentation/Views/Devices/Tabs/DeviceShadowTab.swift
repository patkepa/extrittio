import SwiftUI

struct DeviceShadowTab: View {
    let viewModel: DeviceDetailViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @Environment(AuthViewModel.self) private var authViewModel
    @State private var desiredText = "{}"
    @State private var showDeleteConfirmation = false
    @State private var jsonError: String?
    @State private var isSavingDesired = false
    @State private var isResettingShadow = false

    private var canManageShadows: Bool {
        authViewModel.currentUser?.hasPermission(.shadowsManage) == true
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                if viewModel.isLoadingShadow {
                    LoadingView("Loading shadow...")
                } else if let shadow = viewModel.shadow {
                    shadowContent(shadow)
                } else {
                    EmptyStateView(icon: "doc.text", title: "No Shadow", message: "No device shadow exists yet. Edit desired state to create one.")
                    desiredEditor
                }
            }
            .padding()
        }
        .refreshable { await viewModel.loadShadow(forceRefresh: true) }
        .task {
            await viewModel.loadShadow()
            if let shadow = viewModel.shadow {
                desiredText = prettyJSON(shadow.desired)
            }
        }
        .confirmationDialog("Reset Shadow?", isPresented: $showDeleteConfirmation) {
            Button("Reset", role: .destructive) {
                isResettingShadow = true
                Task {
                    HapticEngine.shared.warning()
                    let success = await viewModel.deleteShadow()
                    isResettingShadow = false
                    if success {
                        toastManager.show(.success("Shadow reset"))
                    } else {
                        toastManager.show(.error("Failed to reset shadow"))
                    }
                }
            }
        } message: {
            Text("This will delete the device shadow. This action cannot be undone.")
        }
    }

    @ViewBuilder
    private func shadowContent(_ shadow: DeviceShadow) -> some View {
        HStack {
            Label("Version \(shadow.version)", systemImage: "clock.arrow.circlepath")
                .font(.caption)
                .foregroundStyle(.secondary)
            Spacer()
            if canManageShadows {
                Button(role: .destructive) {
                    showDeleteConfirmation = true
                } label: {
                    HStack(spacing: Spacing.xs) {
                        if isResettingShadow {
                            ProgressView()
                                .controlSize(.small)
                        } else {
                            Image(systemName: "trash")
                        }
                        Text(isResettingShadow ? "Resetting" : "Reset Shadow")
                    }
                    .font(.caption)
                }
                .disabled(!connectionMonitor.isOnline || isResettingShadow)
                .pressEffect()
            }
        }
        .padding()
        .glassCard()
        .slideIn(delay: 0)

        AdaptiveHStack(spacing: 16) {
            desiredEditor
                .slideIn(delay: 0.07)
            JSONEditorView(
                text: .constant(prettyJSON(shadow.reported)),
                title: "Reported State",
                isEditable: false
            )
            .slideIn(delay: 0.14)
        }

        if !shadow.delta.isEmpty {
            JSONEditorView(
                text: .constant(prettyJSON(shadow.delta)),
                title: "Delta",
                isEditable: false
            )
            .slideIn(delay: 0.21)
        }
    }

    private var desiredEditor: some View {
        VStack(alignment: .leading, spacing: 4) {
            JSONEditorView(
                text: $desiredText,
                title: "Desired State",
                isEditable: canManageShadows,
                disableSave: !connectionMonitor.isOnline || !canManageShadows,
                isSaving: isSavingDesired
            ) { json in
                guard let data = json.data(using: .utf8) else {
                    jsonError = "Invalid text encoding"
                    return
                }
                guard let dict = try? JSONDecoder().decode([String: AnyCodableValue].self, from: data) else {
                    jsonError = "Invalid JSON — check syntax"
                    return
                }
                jsonError = nil
                isSavingDesired = true
                Task {
                    let success = await viewModel.updateDesiredState(dict)
                    isSavingDesired = false
                    if success {
                        toastManager.show(.success("Desired state saved"))
                    } else {
                        toastManager.show(.error("Failed to save desired state"))
                    }
                }
            }
            if let jsonError {
                Text(jsonError)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(.horizontal)
            }
            if !connectionMonitor.isOnline, canManageShadows {
                OfflineActionHint(message: "Desired state cannot be saved while offline.")
            }
        }
    }

    private func prettyJSON(_ dict: [String: AnyCodableValue]) -> String {
        guard let data = try? JSONEncoder().encode(dict),
              let obj = try? JSONSerialization.jsonObject(with: data),
              let pretty = try? JSONSerialization.data(withJSONObject: obj, options: .prettyPrinted),
              let str = String(data: pretty, encoding: .utf8) else { return "{}" }
        return str
    }
}
