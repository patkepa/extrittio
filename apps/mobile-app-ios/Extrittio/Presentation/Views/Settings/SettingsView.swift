import SwiftUI

struct SettingsView: View {
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    let container: DependencyContainer
    @AppStorage("serverAddress") private var serverAddress = ""
    @State private var editedAddress = ""
    @State private var showingLogoutConfirmation = false
    @State private var showingClearCacheConfirmation = false
    @State private var viewModel: SettingsViewModel

    private var editedServerURL: URL? {
        APIConfiguration.current.baseURL(serverAddress: editedAddress)
    }

    private var savedServerURL: URL? {
        APIConfiguration.current.baseURL(serverAddress: serverAddress)
    }

    init(container: DependencyContainer) {
        self.container = container
        self._viewModel = State(initialValue: container.makeSettingsViewModel())
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: Spacing.xl) {
                    // Server Connection section
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        Text("Server Connection")
                            .font(.headline)
                        HStack {
                            Image(systemName: "network")
                                .foregroundStyle(.secondary)
                            TextField("Server IP (e.g. 192.0.2.100)", text: $editedAddress)
                                .autocorrectionDisabled()
                                .textInputAutocapitalization(.never)
                        }
                        .padding()
                        .glassCard()

                        if !editedAddress.isEmpty {
                            HStack {
                                Text("Full URL")
                                    .foregroundStyle(.secondary)
                                Spacer()
                                Text(editedServerURL?.absoluteString ?? "Invalid or insecure server address")
                                    .font(.caption.monospaced())
                                    .foregroundStyle(.secondary)
                            }
                            .padding()
                            .glassCard()
                        }

                        if editedAddress != serverAddress {
                            Button {
                                let address = editedAddress.trimmingCharacters(in: .whitespacesAndNewlines)
                                serverAddress = address
                                viewModel.serverAddress = address
                                Task {
                                    await authViewModel.logout()
                                    toastManager.show(.success("Server changed. Log in again."))
                                }
                            } label: {
                                Text("Save Address")
                                    .bold()
                                    .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(editedServerURL == nil)
                            .pressEffect()
                        }
                    }
                    .slideIn(delay: 0.07)

                    // Connection Info section
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        Text("Connection Info")
                            .font(.headline)
                        VStack(spacing: 0) {
                            settingsInfoRow("Status",
                                value: serverAddress.isEmpty ? "Not configured" : (savedServerURL == nil ? "Invalid" : "Configured"),
                                color: serverAddress.isEmpty || savedServerURL == nil ? .orange : .green)
                            Divider().padding(.horizontal)
                            settingsInfoRow("Port", value: savedServerURL?.port.map(String.init) ?? "Default", color: .secondary)
                            Divider().padding(.horizontal)
                            settingsInfoRow("Protocol", value: savedServerURL?.scheme?.uppercased() ?? "—", color: .secondary)
                        }
                        .glassCard()

                        Button {
                            Task {
                                await viewModel.testConnection()
                                switch viewModel.connectionTestResult {
                                case .success:
                                    toastManager.show(.success("Connected to server"))
                                case .failure(let message):
                                    toastManager.show(.error(message))
                                case nil:
                                    break
                                }
                            }
                        } label: {
                            HStack {
                                if viewModel.isTesting {
                                    ProgressView()
                                        .controlSize(.small)
                                } else {
                                    Image(systemName: "antenna.radiowaves.left.and.right")
                                }
                                Text(viewModel.isTesting ? "Testing…" : "Test Connection")
                                    .bold()
                                    .frame(maxWidth: .infinity)
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(viewModel.isTesting || savedServerURL == nil)
                        .pressEffect()
                    }
                    .slideIn(delay: 0.14)

                    // Management section
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        Text("Management")
                            .font(.headline)

                        VStack(spacing: Spacing.sm) {
                            if can(.firmwareRead) {
                                settingsLink("Firmware Updates", systemImage: "arrow.down.circle") {
                                    FirmwareView(viewModel: container.makeFirmwareViewModel())
                                }
                            }
                            if can(.deviceTypesRead) {
                                settingsLink("Device Types", systemImage: "tag") {
                                    DeviceTypesSettingsView(viewModel: container.makeAdminSettingsViewModel())
                                }
                            }
                            if can(.fleetsRead) {
                                settingsLink("Fleets", systemImage: "folder") {
                                    FleetsSettingsView(viewModel: container.makeAdminSettingsViewModel())
                                }
                            }
                            if can(.usersRead) {
                                settingsLink("Users", systemImage: "person.2") {
                                    UsersSettingsView(viewModel: container.makeAdminSettingsViewModel())
                                }
                            }
                            if can(.rolesRead) {
                                settingsLink("Roles", systemImage: "shield") {
                                    RolesSettingsView(viewModel: container.makeAdminSettingsViewModel())
                                }
                            }
                            if can(.devicesRead) {
                                settingsLink("Certificates", systemImage: "lock") {
                                    CertificatesSettingsView(viewModel: container.makeAdminSettingsViewModel())
                                }
                            }
                            if can(.apiKeysManage) {
                                settingsLink("API Keys", systemImage: "key") {
                                    ApiKeysSettingsView(viewModel: container.makeAdminSettingsViewModel())
                                }
                            }
                        }
                    }
                    .slideIn(delay: 0.21)

                    // Cache section
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        Text("Cache")
                            .font(.headline)
                        VStack(spacing: 0) {
                            settingsInfoRow("Cache Size", value: viewModel.cacheSize, color: .secondary)
                        }
                        .glassCard()

                        Button {
                            HapticEngine.shared.warning()
                            showingClearCacheConfirmation = true
                        } label: {
                            HStack {
                                if viewModel.isClearing {
                                    ProgressView()
                                        .controlSize(.small)
                                } else {
                                    Image(systemName: "trash")
                                }
                                Text(viewModel.isClearing ? "Clearing..." : "Clear Cache")
                                    .bold()
                                    .frame(maxWidth: .infinity)
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.bordered)
                        .tint(.red)
                        .disabled(viewModel.isClearing)
                        .pressEffect()
                    }
                    .slideIn(delay: 0.28)

                    // Logout button
                    Button(role: .destructive) {
                        HapticEngine.shared.warning()
                        showingLogoutConfirmation = true
                    } label: {
                        HStack {
                            Image(systemName: "rectangle.portrait.and.arrow.right")
                            Text("Log Out")
                        }
                        .frame(maxWidth: .infinity)
                        .padding()
                        .glassCard()
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.red)
                    .pressEffect()
                    .slideIn(delay: 0.35)

                    // App version footer
                    VStack(spacing: Spacing.xs) {
                        Text("Extrittio")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                        Text("Version \(viewModel.appVersion) (\(viewModel.buildNumber))")
                            .font(.caption2)
                            .foregroundStyle(.quaternary)
                    }
                    .frame(maxWidth: .infinity)
                    .slideIn(delay: 0.42)
                }
                .padding()
            }
            .navigationTitle("Settings")
            .onAppear {
                editedAddress = serverAddress
            }
            .task {
                await viewModel.loadCacheSize()
            }
            .confirmationDialog("Clear Cache?", isPresented: $showingClearCacheConfirmation) {
                Button("Clear Cache", role: .destructive) {
                    Task {
                        await viewModel.clearCache()
                        toastManager.show(.success("Cache cleared"))
                    }
                }
            } message: {
                Text("All cached data will be removed. Data will be re-fetched from the server.")
            }
            .confirmationDialog("Log Out?", isPresented: $showingLogoutConfirmation) {
                Button("Log Out", role: .destructive) {
                    Task { await authViewModel.logout() }
                }
            } message: {
                Text("You will need to log in again.")
            }
        }
    }

    private func settingsInfoRow(_ label: String, value: String, color: Color) -> some View {
        HStack {
            Text(label)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .foregroundStyle(color)
        }
        .padding()
    }

    private func can(_ permission: PermissionKey) -> Bool {
        authViewModel.currentUser?.hasPermission(permission) == true
    }

    private func settingsLink<Destination: View>(_ title: String, systemImage: String, @ViewBuilder destination: @escaping () -> Destination) -> some View {
        NavigationLink {
            destination()
        } label: {
            HStack {
                Label(title, systemImage: systemImage)
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding()
            .glassCard()
        }
        .buttonStyle(.plain)
    }
}
