import SwiftUI

struct DeviceConfigTab: View {
    let viewModel: DeviceDetailViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @Environment(AuthViewModel.self) private var authViewModel
    @State private var configText = "{}"
    @State private var originalConfig = "{}"
    @State private var jsonError: String?
    @State private var shakeOffset: CGFloat = 0
    @State private var isSavingConfig = false

    private var isDirty: Bool { configText != originalConfig }
    private var canManageDevices: Bool {
        authViewModel.currentUser?.hasPermission(.devicesManage) == true
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                if viewModel.isLoadingConfig {
                    LoadingView("Loading configuration...")
                } else {
                    VStack(alignment: .leading, spacing: 4) {
                        JSONEditorView(
                            text: $configText,
                            title: "Device Configuration",
                            isEditable: canManageDevices,
                            disableSave: !connectionMonitor.isOnline || !canManageDevices,
                            isSaving: isSavingConfig
                        ) { json in
                            guard let data = json.data(using: .utf8) else {
                                jsonError = "Invalid text encoding"
                                triggerShake()
                                return
                            }
                            guard let dict = try? JSONDecoder().decode([String: AnyCodableValue].self, from: data) else {
                                jsonError = "Invalid JSON — check syntax"
                                HapticEngine.shared.error()
                                triggerShake()
                                return
                            }
                            jsonError = nil
                            isSavingConfig = true
                            Task {
                                let success = await viewModel.updateConfig(dict)
                                isSavingConfig = false
                                if success {
                                    originalConfig = configText
                                    toastManager.show(.success("Config saved"))
                                } else {
                                    toastManager.show(.error("Failed to save config"))
                                }
                            }
                        }
                        .offset(x: shakeOffset)
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .strokeBorder(
                                    isDirty ? Color.accentColor.opacity(0.5) : Color.clear,
                                    lineWidth: 1.5
                                )
                        )

                        if let jsonError {
                            Text(jsonError)
                                .font(.caption)
                                .foregroundStyle(.red)
                                .padding(.horizontal)
                                .transition(.opacity.combined(with: .move(edge: .top)))
                        }

                        if !connectionMonitor.isOnline, canManageDevices {
                            OfflineActionHint(message: "Configuration changes cannot be saved while offline.")
                        }
                    }

                    HStack(spacing: 8) {
                        Image(systemName: "info.circle")
                            .foregroundStyle(.blue)
                        Text("Configuration uses merge semantics — only provided keys are updated.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding()
                    .glassCard()
                }
            }
            .padding()
        }
        .refreshable { await viewModel.loadConfig(forceRefresh: true) }
        .task {
            await viewModel.loadConfig()
            updateConfigText()
        }
        .onChange(of: viewModel.config) { _, _ in updateConfigText() }
    }

    private func updateConfigText() {
        guard let data = try? JSONEncoder().encode(viewModel.config),
              let obj = try? JSONSerialization.jsonObject(with: data),
              let pretty = try? JSONSerialization.data(withJSONObject: obj, options: .prettyPrinted),
              let str = String(data: pretty, encoding: .utf8) else { return }
        configText = str
        originalConfig = str
    }

    private func triggerShake() {
        withAnimation(nil) { shakeOffset = 0 }
        withAnimation(.interpolatingSpring(stiffness: 600, damping: 10)) {
            shakeOffset = -8
        }
        Task {
            try? await Task.sleep(for: .milliseconds(300))
            withAnimation(.interpolatingSpring(stiffness: 600, damping: 10)) {
                shakeOffset = 0
            }
        }
    }
}
