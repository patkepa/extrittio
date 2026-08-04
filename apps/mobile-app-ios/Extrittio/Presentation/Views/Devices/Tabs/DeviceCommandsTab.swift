import SwiftUI

struct DeviceCommandsTab: View {
    let viewModel: DeviceDetailViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @Environment(AuthViewModel.self) private var authViewModel
    @State private var commandName = ""
    @State private var commandParams = ""
    @State private var isSending = false
    @State private var jsonError: String?
    @State private var shakeOffset: CGFloat = 0

    var body: some View {
        ScrollView {
            AdaptiveHStack(spacing: 16) {
                if canSendCommands {
                    sendCommandSection
                }
                commandHistorySection
            }
            .padding()
        }
        .refreshable { await viewModel.loadCommands(forceRefresh: true) }
        .task { await viewModel.loadCommands() }
    }

    private var canSendCommands: Bool {
        authViewModel.currentUser?.hasPermission(.commandsSend) == true
    }

    private var sendCommandSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Send Command")
                .font(.headline)

            if !connectionMonitor.isOnline {
                OfflineActionHint(message: "Commands cannot be sent while offline.")
            }

            TextField("Command name (e.g. restart)", text: $commandName)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .padding()
                .glassCard()

            TextField("Parameters JSON (optional)", text: $commandParams)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .font(.system(.body, design: .monospaced))
                .padding()
                .glassCard()
                .offset(x: shakeOffset)

            if let jsonError {
                Text(jsonError)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }

            Button {
                jsonError = nil
                let params: [String: AnyCodableValue]?
                if !commandParams.isEmpty {
                    guard let data = commandParams.data(using: .utf8),
                          let dict = try? JSONDecoder().decode([String: AnyCodableValue].self, from: data) else {
                        withAnimation(nil) {
                            shakeOffset = 0
                        }
                        jsonError = "Invalid JSON parameters"
                        HapticEngine.shared.error()
                        withAnimation(.interpolatingSpring(stiffness: 600, damping: 10)) {
                            shakeOffset = -8
                        }
                        Task {
                            try? await Task.sleep(for: .milliseconds(300))
                            withAnimation(.interpolatingSpring(stiffness: 600, damping: 10)) {
                                shakeOffset = 0
                            }
                        }
                        return
                    }
                    params = dict
                } else {
                    params = nil
                }
                HapticEngine.shared.impact(.light)
                isSending = true
                let capturedCommand = commandName
                Task {
                    let success = await viewModel.sendCommand(command: capturedCommand, params: params)
                    commandName = ""
                    commandParams = ""
                    isSending = false
                    if success {
                        toastManager.show(.success("Command '\(capturedCommand)' sent"))
                    } else {
                        toastManager.show(.error("Failed to send command '\(capturedCommand)'"))
                    }
                }
            } label: {
                ActionProgressLabel(title: "Send", systemImage: "paperplane", isLoading: isSending, loadingTitle: "Sending")
            }
            .buttonStyle(.borderedProminent)
            .disabled(commandName.isEmpty || isSending || !connectionMonitor.isOnline)
            .pressEffect()
        }
        .padding()
        .glassCard()
    }

    @ViewBuilder
    private var commandHistorySection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Command History")
                .font(.headline)

            if viewModel.isLoadingCommands && viewModel.commands.isEmpty {
                LoadingView("Loading commands...")
            } else if viewModel.commands.isEmpty {
                Text("No commands sent yet.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(Array(viewModel.commands.enumerated()), id: \.element.id) { index, cmd in
                    commandRow(cmd)
                        .slideIn(delay: Double(index) * 0.05)
                    Divider()
                }
            }
        }
        .padding()
        .glassCard()
    }

    private func commandRow(_ cmd: CommandRecord) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(cmd.command)
                    .font(.callout.bold().monospaced())
                Spacer()
                commandStatusBadge(cmd.status)
            }
            Text(String.formattedTimestamp(cmd.createdAt))
                .font(.caption2)
                .foregroundStyle(.secondary)
            if let payload = cmd.responsePayload, !payload.isEmpty {
                Text(prettyJSON(payload))
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                    .background(.ultraThinMaterial, in: .rect(cornerRadius: 8))
            }
        }
        .padding(.vertical, 4)
    }

    private func commandStatusBadge(_ status: String) -> some View {
        Text(status.replacingOccurrences(of: "_", with: " ").capitalized)
            .font(.caption.bold())
            .foregroundStyle(statusColor(status))
            .padding(.horizontal, 8)
            .padding(.vertical, 2)
            .glassCard()
    }

    private func statusColor(_ status: String) -> Color {
        switch status {
        case "success": .green
        case "failed": .red
        case "timed_out": .orange
        case "pending": .blue
        default: .gray
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
