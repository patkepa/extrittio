import SwiftUI

@MainActor
struct NearbyDevicePanel: View {
    private struct PendingOutletChange {
        let target: NearbyDeviceRelayTarget
        let previousState: Bool
    }

    @Environment(\.dismiss) private var dismiss
    @State private var scanner: NearbyDeviceScanner
    @State private var isPulsing = false
    @State private var presentationDetent = PresentationDetent.medium
    @State private var leftOutletIsOn = false
    @State private var rightOutletIsOn = false
    @State private var pendingOutletChange: PendingOutletChange?
    @State private var sound = NearbyDeviceSound.locate
    @State private var soundRepeatCount = 1

    init(scanner: NearbyDeviceScanner) {
        _scanner = State(initialValue: scanner)
    }

    var body: some View {
        NavigationStack {
            GeometryReader { geometry in
                ScrollView {
                    VStack(spacing: Spacing.xl) {
                        VStack(spacing: Spacing.xl) {
                            if !deviceIsIdentified {
                                proximityHero
                            }
                            stateContent
                            if showsExpandedDeviceList {
                                nearbyDevicesList
                            }
                            if !deviceIsIdentified, presentationDetent == .large {
                                privacyNote
                            }
                        }
                        .frame(
                            minHeight: showsExpandedDeviceList
                                ? max(geometry.size.height - Spacing.lg * 2, 0)
                                : nil,
                            alignment: .top
                        )

                        if showsExpandedDeviceList {
                            contactThresholdControl
                        }
                    }
                    .frame(maxWidth: 560)
                    .padding(Spacing.lg)
                    .frame(maxWidth: .infinity)
                }
            }
            .background(Color(.systemGroupedBackground))
            .navigationTitle(deviceIsIdentified ? "" : "Tap into a device")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
        .task { scanner.startScanning() }
        .onDisappear { scanner.stopScanning() }
        .onChange(of: scanner.state) { _, state in
            if case .identified = state {
                presentationDetent = .large
                HapticEngine.shared.success()
            }
        }
        .onChange(of: scanner.writeState) { _, state in
            switch state {
            case .succeeded:
                pendingOutletChange = nil
                HapticEngine.shared.success()
            case .failed:
                rollbackPendingOutletChange()
            case .idle, .writing:
                break
            }
        }
        .presentationDetents([.medium, .large], selection: $presentationDetent)
        .presentationDragIndicator(.visible)
    }

    private var deviceIsIdentified: Bool {
        if case .identified = scanner.state {
            return true
        }
        return false
    }

    private var showsExpandedDeviceList: Bool {
        guard presentationDetent == .large else { return false }
        if case .scanning = scanner.state {
            return true
        }
        return false
    }

    private var proximityHero: some View {
        ZStack {
            Circle()
                .stroke(Color.accentColor.opacity(0.12), lineWidth: 2)
                .frame(width: 176, height: 176)
                .scaleEffect(isPulsing && scanner.state.isSearching ? 1.08 : 0.92)
            Circle()
                .fill(Color.accentColor.opacity(0.1))
                .frame(width: 128, height: 128)
            Image(systemName: heroIcon)
                .font(.system(size: 48, weight: .medium))
                .foregroundStyle(heroColor)
        }
        .frame(height: 184)
        .accessibilityHidden(true)
        .onAppear {
            withAnimation(.easeInOut(duration: 1.2).repeatForever(autoreverses: true)) {
                isPulsing = true
            }
        }
    }

    @ViewBuilder
    private var stateContent: some View {
        switch scanner.state {
        case .idle, .preparing:
            progressCard(
                title: "Preparing Bluetooth",
                message: "Extrittio is getting ready to look for nearby devices."
            )
        case .scanning:
            scanningCard
        case .connecting(let candidate):
            progressCard(
                title: "Connecting",
                message: "Reading identity from \(candidate.displayName)…"
            )
        case .reading:
            progressCard(
                title: "Reading Device",
                message: "The short-range connection is retrieving device information."
            )
        case .identified(let info, let candidate):
            identityCard(info: info, candidate: candidate)
        case .unavailable(let availability):
            unavailableCard(availability)
        case .failed(let message):
            failureCard(message: message)
        }
    }

    private var scanningCard: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Find a device")
                    .font(.title3.bold())
                Text(
                    presentationDetent == .large
                        ? "Bring your iPhone close to connect automatically, or select a nearby device below."
                        : "Bring your iPhone close to connect automatically, or open the full view to select a nearby device."
                )
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(Spacing.lg)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard(.elevated)
    }

    private var nearbyDevices: [NearbyDeviceCandidate] {
        scanner.nearbyDevices.filter { $0.proximity != .detected }
    }

    @ViewBuilder
    private var nearbyDevicesList: some View {
        Group {
            if nearbyDevices.isEmpty {
                HStack(spacing: Spacing.sm) {
                    ProgressView()
                    Text("Scanning for devices close by…")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .padding(Spacing.lg)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(nearbyDevices.enumerated()), id: \.element.id) { index, device in
                        nearbyDeviceRow(device)
                        if index < nearbyDevices.count - 1 {
                            Divider()
                                .padding(.leading, 48)
                        }
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard(.elevated)
    }

    private func nearbyDeviceRow(_ device: NearbyDeviceCandidate) -> some View {
        let proximity = device.proximity(contactRSSIThreshold: scanner.contactRSSIThreshold)

        return Button {
            scanner.connect(to: device)
        } label: {
            HStack(spacing: Spacing.md) {
                Image(systemName: "sensor.tag.radiowaves.forward")
                    .font(.title3.weight(.medium))
                    .foregroundStyle(.secondary)
                    .frame(width: 24, height: 24)

                VStack(alignment: .leading, spacing: 3) {
                    Text(device.displayName)
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    Text("Signal \(device.rssi) dBm")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }

                Spacer(minLength: Spacing.sm)

                Image(systemName: "chevron.right")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(.tertiary)
            }
            .padding(Spacing.md)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Connect to \(device.displayName)")
        .accessibilityValue("\(proximityLabel(proximity)), signal \(device.rssi) dBm")
        .accessibilityHint("Connects directly without waiting for contact range")
    }

    private var contactThresholdControl: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack {
                Label("Contact threshold", systemImage: "dot.radiowaves.left.and.right")
                    .font(.subheadline.weight(.semibold))
                Spacer()
                Text("\(scanner.contactRSSIThreshold) dBm")
                    .font(.subheadline.monospacedDigit().weight(.semibold))
            }

            Slider(
                value: contactThresholdBinding,
                in: Double(NearbyDeviceProximity.supportedContactRSSIThresholds.lowerBound) ...
                    Double(NearbyDeviceProximity.supportedContactRSSIThresholds.upperBound),
                step: 1
            )

            Text("Raise the threshold to require the iPhone to be closer before connecting automatically.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(Spacing.md)
        .background(Color.accentColor.opacity(0.07), in: .rect(cornerRadius: Spacing.md))
    }

    private var contactThresholdBinding: Binding<Double> {
        Binding(
            get: { Double(scanner.contactRSSIThreshold) },
            set: { scanner.setContactRSSIThreshold(Int($0.rounded())) }
        )
    }

    private func progressCard(title: String, message: String) -> some View {
        VStack(spacing: Spacing.md) {
            ProgressView()
                .controlSize(.large)
            Text(title)
                .font(.title3.bold())
            Text(message)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(Spacing.xl)
        .frame(maxWidth: .infinity)
        .glassCard(.elevated)
    }

    private func identityCard(info: NearbyDeviceInfo, candidate: NearbyDeviceCandidate) -> some View {
        VStack(spacing: Spacing.xl) {
            identifiedDeviceTitle(info: info, candidate: candidate)

            if info.isDoubleSocket {
                doubleSocketPowerControls
                writeStatus
                doubleSocketSecondaryControls
            } else {
                identificationControl
            }

            Button {
                scanner.startScanning()
            } label: {
                Label("Scan Again", systemImage: "arrow.clockwise")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
            .pressEffect()

            if presentationDetent == .large {
                privacyNote
            }
            deviceDetails(info: info, candidate: candidate)
        }
        .frame(maxWidth: .infinity)
    }

    private func identifiedDeviceTitle(
        info: NearbyDeviceInfo,
        candidate: NearbyDeviceCandidate
    ) -> some View {
        VStack(spacing: Spacing.sm) {
            Text(candidate.displayName)
                .font(.title2.bold())
                .multilineTextAlignment(.center)

            if candidate.displayName != info.model {
                Text(info.model)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            HStack(spacing: Spacing.sm) {
                Circle()
                    .fill(.green)
                    .frame(width: 7, height: 7)
                Text("Connected nearby")
                Text("·")
                Text("\(candidate.rssi) dBm")
                    .monospacedDigit()
            }
            .font(.caption.monospacedDigit().weight(.medium))
            .foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
    }

    private var doubleSocketPowerControls: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Outlet power")
                    .font(.headline)
                Text("Tap a gang to switch it on or off.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            GlassEffectContainer(spacing: Spacing.md) {
                HStack(spacing: Spacing.md) {
                    outletButton(
                        title: "Left",
                        target: .left,
                        isOn: leftOutletIsOn
                    )
                    outletButton(
                        title: "Right",
                        target: .right,
                        isOn: rightOutletIsOn
                    )
                }
            }

            Text("State reflects the last command sent during this nearby connection.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.easeOut(duration: 0.2), value: leftOutletIsOn)
        .animation(.easeOut(duration: 0.2), value: rightOutletIsOn)
    }

    private var doubleSocketSecondaryControls: some View {
        VStack(alignment: .leading, spacing: Spacing.xl) {
            VStack(alignment: .leading, spacing: Spacing.md) {
                doubleSocketControlHeader(
                    title: "Play sound",
                    detail: "Use a sound to locate or demonstrate the device.",
                    systemImage: "speaker.wave.2.fill"
                )

                Picker("Sound", selection: $sound) {
                    ForEach(NearbyDeviceSound.allCases) { option in
                        Text(option.title).tag(option)
                    }
                }
                .pickerStyle(.segmented)
                .disabled(scanner.writeState.isWriting)

                Stepper(value: $soundRepeatCount, in: NearbyDeviceCommand.supportedSoundRepeatCounts) {
                    HStack {
                        Text("Repeats")
                        Spacer()
                        Text(soundRepeatCount.formatted())
                            .fontWeight(.semibold)
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                    }
                }
                .disabled(scanner.writeState.isWriting)

                Button {
                    guard let command = NearbyDeviceCommand.sound(
                        type: sound,
                        repeatCount: soundRepeatCount
                    ) else { return }
                    scanner.send(command)
                } label: {
                    Label("Play \(sound.title)", systemImage: "speaker.wave.2.fill")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(scanner.writeState.isWriting)
                .pressEffect()
            }

            Divider()

            Button {
                scanner.requestIdentificationFeedback()
            } label: {
                Label("Flash Identification Lights", systemImage: "light.beacon.max.fill")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
            .disabled(scanner.writeState.isWriting)
            .pressEffect()

            HStack(alignment: .top, spacing: Spacing.sm) {
                Image(systemName: "shield.lefthalf.filled")
                    .foregroundStyle(.secondary)
                Text("Controls pause automatically during alerts and firmware updates.")
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .font(.subheadline)
        .padding(Spacing.lg)
        .background(Color(.secondarySystemGroupedBackground), in: .rect(cornerRadius: Spacing.lg))
        .overlay {
            RoundedRectangle(cornerRadius: Spacing.lg, style: .continuous)
                .stroke(Color.primary.opacity(0.06), lineWidth: 1)
        }
        .animation(.easeOut(duration: 0.2), value: scanner.writeState)
    }

    private func outletButton(
        title: String,
        target: NearbyDeviceRelayTarget,
        isOn: Bool
    ) -> some View {
        outletButtonControl(title: title, target: target, isOn: isOn)
    }

    private func outletButtonControl(
        title: String,
        target: NearbyDeviceRelayTarget,
        isOn: Bool
    ) -> some View {
        Button {
            sendOutletCommand(target: target, isOn: !isOn)
        } label: {
            VStack(alignment: .leading, spacing: Spacing.md) {
                HStack {
                    Image(systemName: isOn ? "powerplug.fill" : "powerplug")
                        .font(.title2.weight(.semibold))

                    Spacer()

                    Label(
                        isOn ? "On" : "Off",
                        systemImage: isOn ? "checkmark.circle.fill" : "power.circle"
                    )
                    .font(.caption.weight(.bold))
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xs)
                    .background(.white.opacity(0.18), in: .capsule)
                }

                Text("\(title) gang")
                    .font(.title3.bold())

                Text(isOn ? "Powered" : "Not powered")
                    .font(.caption.weight(.medium))
            }
            .foregroundStyle(isOn ? .white : .primary)
            .padding(Spacing.md)
            .frame(maxWidth: .infinity, minHeight: 124, alignment: .leading)
            .background(outletButtonFillColor(isOn: isOn), in: .rect(cornerRadius: Spacing.md))
            .overlay {
                RoundedRectangle(cornerRadius: Spacing.md, style: .continuous)
                    .stroke(outletButtonBorderColor(isOn: isOn), lineWidth: 1.5)
            }
            .contentShape(.rect)
        }
        .disabled(scanner.writeState.isWriting)
        .accessibilityLabel("\(title) gang power")
        .accessibilityValue(isOn ? "On" : "Off")
        .accessibilityHint(isOn ? "Turns the outlet off" : "Turns the outlet on")
        .pressEffect()
    }

    private func outletButtonFillColor(isOn: Bool) -> Color {
        isOn ? .green.opacity(0.82) : Color(.systemGray4).opacity(0.55)
    }

    private func outletButtonBorderColor(isOn: Bool) -> Color {
        isOn ? .green.opacity(0.95) : Color(.systemGray)
    }

    private func outletState(for target: NearbyDeviceRelayTarget) -> Bool {
        switch target {
        case .left:
            leftOutletIsOn
        case .right:
            rightOutletIsOn
        case .all:
            leftOutletIsOn && rightOutletIsOn
        }
    }

    private func setOutletState(_ isOn: Bool, for target: NearbyDeviceRelayTarget) {
        switch target {
        case .left:
            leftOutletIsOn = isOn
        case .right:
            rightOutletIsOn = isOn
        case .all:
            leftOutletIsOn = isOn
            rightOutletIsOn = isOn
        }
    }

    private func sendOutletCommand(target: NearbyDeviceRelayTarget, isOn: Bool) {
        guard !scanner.writeState.isWriting else { return }

        let previousState = outletState(for: target)
        guard previousState != isOn else { return }

        pendingOutletChange = PendingOutletChange(
            target: target,
            previousState: previousState
        )
        setOutletState(isOn, for: target)
        scanner.send(.relay(target: target, state: isOn ? .on : .off))
    }

    private func rollbackPendingOutletChange() {
        guard let pendingOutletChange else { return }
        setOutletState(pendingOutletChange.previousState, for: pendingOutletChange.target)
        self.pendingOutletChange = nil
    }

    private func doubleSocketControlHeader(
        title: String,
        detail: String,
        systemImage: String
    ) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: systemImage)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(Color.accentColor)
                .frame(width: 22, height: 22)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(title)
                    .font(.subheadline.weight(.semibold))
                Text(detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var identificationControl: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Identify device")
                .font(.headline)
            Text("Ask the connected device to repeat its visual identification feedback.")
                .font(.caption)
                .foregroundStyle(.secondary)
            Button {
                scanner.requestIdentificationFeedback()
            } label: {
                Label("Identify", systemImage: "light.beacon.max.fill")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(scanner.writeState.isWriting)
            .pressEffect()

            writeStatus
        }
        .font(.subheadline)
        .padding(Spacing.md)
        .background(Color.accentColor.opacity(0.07), in: .rect(cornerRadius: Spacing.md))
    }

    @ViewBuilder
    private var writeStatus: some View {
        switch scanner.writeState {
        case .idle:
            EmptyView()
        case .writing(let request):
            writeStatusLabel(
                request.progressMessage,
                systemImage: "arrow.up.circle",
                color: .secondary
            )
        case .succeeded(let request):
            writeStatusLabel(
                request.successMessage,
                systemImage: "checkmark.circle.fill",
                color: .green
            )
        case .failed(_, let error):
            writeStatusLabel(
                error,
                systemImage: "exclamationmark.triangle.fill",
                color: .red
            )
        }
    }

    private func writeStatusLabel(
        _ message: String,
        systemImage: String,
        color: Color
    ) -> some View {
        Label(message, systemImage: systemImage)
            .font(.caption.weight(.medium))
            .foregroundStyle(color)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Spacing.md)
            .padding(.vertical, Spacing.sm)
            .background(color.opacity(0.09), in: .rect(cornerRadius: Spacing.sm))
            .transition(.opacity.combined(with: .move(edge: .top)))
    }

    private func identityRow(_ label: String, value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
            Text(label)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .fontWeight(.medium)
                .multilineTextAlignment(.trailing)
                .textSelection(.enabled)
        }
        .font(.subheadline)
        .padding(.vertical, Spacing.sm)
        .accessibilityElement(children: .combine)
    }

    private func deviceDetails(
        info: NearbyDeviceInfo,
        candidate: NearbyDeviceCandidate
    ) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Label("Device details", systemImage: "info.circle")
                .font(.headline)

            VStack(spacing: 0) {
                identityRow("Device ID", value: info.deviceId)
                Divider()
                identityRow("Model", value: info.model)
                Divider()
                identityRow("Firmware", value: info.firmwareVersion)
                Divider()
                identityRow("Transport", value: info.transport)
                Divider()
                identityRow("BLE signal", value: "\(candidate.rssi) dBm")
            }
        }
        .padding(Spacing.lg)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassCard()
    }

    private func unavailableCard(_ availability: NearbyDeviceAvailability) -> some View {
        let content = unavailableContent(availability)
        return VStack(spacing: Spacing.lg) {
            ContentUnavailableView(
                content.title,
                systemImage: content.icon,
                description: Text(content.message)
            )
            Button("Try Again") { scanner.startScanning() }
                .buttonStyle(.borderedProminent)
                .pressEffect()
        }
        .padding(Spacing.lg)
        .frame(maxWidth: .infinity)
        .glassCard(.elevated)
    }

    private func failureCard(message: String) -> some View {
        VStack(spacing: Spacing.lg) {
            ContentUnavailableView(
                "Could Not Read Device",
                systemImage: "exclamationmark.triangle",
                description: Text(message)
            )
            Button("Scan Again") { scanner.startScanning() }
                .buttonStyle(.borderedProminent)
                .pressEffect()
        }
        .padding(Spacing.lg)
        .frame(maxWidth: .infinity)
        .glassCard(.elevated)
    }

    private var privacyNote: some View {
        Label(
            "Bluetooth exchanges device identity and direct local commands. Network credentials and security keys are never shared.",
            systemImage: "lock.shield"
        )
        .font(.footnote)
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Spacing.sm)
    }

    private var heroIcon: String {
        if case .identified = scanner.state {
            return "checkmark"
        }
        return "dot.radiowaves.left.and.right"
    }

    private var heroColor: Color {
        if case .identified = scanner.state {
            return .green
        }
        return .accentColor
    }

    private func proximityLabel(_ proximity: NearbyDeviceProximity) -> String {
        switch proximity {
        case .detected: "Detected"
        case .near: "Near"
        case .contact: "Very close"
        }
    }

    private func proximityColor(_ proximity: NearbyDeviceProximity) -> Color {
        switch proximity {
        case .detected: .secondary
        case .near: .orange
        case .contact: .green
        }
    }

    private func unavailableContent(
        _ availability: NearbyDeviceAvailability
    ) -> (title: String, message: String, icon: String) {
        switch availability {
        case .bluetoothOff:
            ("Bluetooth Is Off", "Turn on Bluetooth in Control Center or Settings, then try again.", "wave.3.right.slash")
        case .unauthorized:
            ("Bluetooth Access Needed", "Allow Bluetooth access for Extrittio in Settings to connect to nearby devices.", "hand.raised")
        case .unsupported:
            ("Bluetooth Unavailable", "This device cannot scan for Bluetooth Low Energy peripherals.", "antenna.radiowaves.left.and.right.slash")
        }
    }
}
