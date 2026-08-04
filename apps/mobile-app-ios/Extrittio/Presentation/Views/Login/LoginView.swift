import SwiftUI

struct LoginView: View {
    @Environment(AuthViewModel.self) private var authViewModel
    @AppStorage("serverAddress") private var serverAddress = ""
    @State private var username = ""
    @State private var password = ""
    @State private var showingServerConfig = false
    @State private var showContent = false
    @State private var shakeOffset: CGFloat = 0

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                Spacer()

                Image(systemName: "sensor.tag.radiowaves.forward.fill")
                    .font(.system(size: 64))
                    .foregroundStyle(.tint)
                    .scaleEffect(showContent ? 1 : 0.8)
                    .opacity(showContent ? 1 : 0)

                Text("Extrittio")
                    .font(.largeTitle.bold())
                    .slideIn(delay: 0.2)

                Text("IoT Hub")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                    .slideIn(delay: 0.3)

                VStack(spacing: 16) {
                    TextField("Username", text: $username)
                        .textContentType(.username)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .padding()
                        .glassCard()
                        .slideIn(delay: 0.4)

                    SecureField("Password", text: $password)
                        .textContentType(.password)
                        .padding()
                        .glassCard()
                        .slideIn(delay: 0.5)
                }
                .padding(.horizontal, 32)
                .offset(x: shakeOffset)

                if let error = authViewModel.errorMessage {
                    HStack(spacing: 8) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundStyle(.red)
                        Text(error)
                            .font(.callout)
                            .foregroundStyle(.red)
                    }
                    .padding()
                    .frame(maxWidth: .infinity)
                    .glassCard()
                    .padding(.horizontal, 32)
                }

                Button {
                    HapticEngine.shared.impact(.light)
                    Task { await authViewModel.login(username: username, password: password) }
                } label: {
                    if authViewModel.isLoading {
                        ProgressView()
                            .frame(maxWidth: .infinity)
                    } else {
                        Text("Log In")
                            .font(.headline)
                            .frame(maxWidth: .infinity)
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(
                    username.isEmpty ||
                        password.isEmpty ||
                        authViewModel.isLoading ||
                        APIConfiguration.current.baseURL(serverAddress: serverAddress) == nil
                )
                .padding(.horizontal, 32)
                .pressEffect()
                .slideIn(delay: 0.6)

                if serverAddress.isEmpty {
                    Text("Server address not configured")
                        .font(.caption)
                        .foregroundStyle(.orange)
                }

                Button {
                    showingServerConfig = true
                } label: {
                    Label("Server Settings", systemImage: "network")
                        .font(.callout)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                        .glassCard()
                }

                Spacer()
            }
            .frame(maxWidth: 460)
            .frame(maxWidth: .infinity)
            .sheet(isPresented: $showingServerConfig) {
                ServerConfigSheet(serverAddress: $serverAddress)
            }
            .onAppear {
                withAnimation(AppAnimation.gentle.animation.delay(0.1)) {
                    showContent = true
                }
            }
            .onChange(of: authViewModel.errorMessage) { _, newValue in
                guard newValue != nil else { return }
                HapticEngine.shared.error()
                withAnimation(AppAnimation.quick.animation) {
                    shakeOffset = 6
                }
                withAnimation(AppAnimation.quick.animation.delay(0.08)) {
                    shakeOffset = -6
                }
                withAnimation(AppAnimation.quick.animation.delay(0.16)) {
                    shakeOffset = 4
                }
                withAnimation(AppAnimation.quick.animation.delay(0.24)) {
                    shakeOffset = 0
                }
            }
        }
    }
}

struct ServerConfigSheet: View {
    @Binding var serverAddress: String
    @Environment(\.dismiss) private var dismiss
    @State private var editedAddress: String = ""

    private var normalizedAddress: String {
        editedAddress.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var serverURL: URL? {
        APIConfiguration.current.baseURL(serverAddress: normalizedAddress)
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 20) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Server Address")
                            .font(.headline)
                        TextField("e.g. 192.0.2.100", text: $editedAddress)
                            .textContentType(.URL)
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                            .padding()
                            .glassCard()

                        if !normalizedAddress.isEmpty, serverURL == nil {
                            Text(APIConfiguration.current.allowsInsecureHTTP
                                ? "Enter a valid HTTP or HTTPS server address."
                                : "Enter a valid HTTPS server address.")
                                .font(.caption)
                                .foregroundStyle(.red)
                        }
                    }

                    HStack(spacing: 8) {
                        Image(systemName: "info.circle")
                            .foregroundStyle(.blue)
                        Text("Enter the host or URL of your Extrittio server. The app uses this build's configured API scheme, port, and base path.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding()
                    .glassCard()
                }
                .padding()
            }
            .navigationTitle("Server Configuration")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        serverAddress = normalizedAddress
                        dismiss()
                    }
                    .disabled(serverURL == nil)
                }
            }
            .onAppear { editedAddress = serverAddress }
        }
    }
}
