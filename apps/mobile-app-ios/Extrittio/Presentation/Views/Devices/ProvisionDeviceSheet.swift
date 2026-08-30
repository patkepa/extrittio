import SwiftUI

@MainActor
struct ProvisionDeviceSheet: View {
    @Environment(\.dismiss) private var dismiss
    @State private var model: ProvisionDeviceViewModel
    @State private var scanner: NearbyDeviceScanner
    let onProvisioned: (Device) -> Void

    init(
        model: ProvisionDeviceViewModel,
        scanner: NearbyDeviceScanner,
        onProvisioned: @escaping (Device) -> Void
    ) {
        _model = State(initialValue: model)
        _scanner = State(initialValue: scanner)
        self.onProvisioned = onProvisioned
    }

    var body: some View {
        NavigationStack {
            Form {
                stepHeader
                stepContent
                if let errorMessage = model.errorMessage {
                    Section {
                        Label(errorMessage, systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(.red)
                    }
                }
            }
            .formStyle(.grouped)
            .navigationTitle("Provision Device")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { toolbarContent }
        }
        .task { await model.load() }
        .onChange(of: model.step) { _, step in
            switch step {
            case .scan:
                scanner.startScanning()
            case .transfer:
                if let payload = model.prepared?.payload {
                    scanner.provision(payload: payload)
                }
            case .completed, .configure, .loading, .registering:
                break
            }
        }
        .onChange(of: scanner.provisioningState) { _, state in
            switch state {
            case .succeeded:
                model.markCompleted()
                HapticEngine.shared.success()
            case .failed(let message):
                model.showTransferError(message)
            case .idle, .transferring:
                break
            }
        }
        .onDisappear { scanner.stopScanning() }
        .interactiveDismissDisabled(model.registeredDevice != nil && model.step != .completed)
    }

    private var stepHeader: some View {
        Section {
            HStack(spacing: Spacing.md) {
                Image(systemName: stepIcon)
                    .font(.title2)
                    .foregroundStyle(Color.accentColor)
                    .frame(width: 36, height: 36)
                    .background(Color.accentColor.opacity(0.12), in: .circle)
                VStack(alignment: .leading, spacing: 2) {
                    Text(stepTitle)
                        .font(.headline)
                    Text(stepMessage)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }
            .accessibilityElement(children: .combine)
        }
    }

    @ViewBuilder
    private var stepContent: some View {
        switch model.step {
        case .loading:
            Section { ProgressView("Loading blueprints and fleets…") }
        case .configure:
            configurationSections
        case .scan:
            scannerSections
        case .registering:
            Section {
                ProgressView("Creating the device and issuing credentials…")
            }
        case .transfer:
            transferSections
        case .completed:
            completionSections
        }
    }

    @ViewBuilder
    private var configurationSections: some View {
        Section("Device") {
            TextField("Device name", text: Bindable(model).name)
            Picker("Blueprint", selection: blueprintSelection) {
                ForEach(model.blueprints) { blueprint in
                    Text(blueprint.name).tag(blueprint.id)
                }
            }
            .disabled(model.blueprints.isEmpty)
            Picker("Fleet", selection: Bindable(model).selectedFleetId) {
                Text("No fleet").tag(nil as Int?)
                ForEach(model.fleets) { fleet in
                    Text(fleet.name).tag(fleet.id as Int?)
                }
            }
        }

        if model.revision?.supportsConfiguration == true {
            Section {
                TextEditor(text: Bindable(model).configurationText)
                    .font(.body.monospaced())
                    .frame(minHeight: 100)
            } header: {
                Text("Configuration override")
            } footer: {
                Text("Optional JSON validated against the selected blueprint.")
            }
        }
    }

    @ViewBuilder
    private var scannerSections: some View {
        switch scanner.state {
        case .idle, .preparing:
            Section { ProgressView("Preparing Bluetooth…") }
        case .scanning:
            Section("Nearby devices") {
                if scanner.nearbyDevices.isEmpty {
                    ProgressView("Looking for devices advertising the Extrittio service…")
                } else {
                    ForEach(scanner.nearbyDevices) { candidate in
                        Button {
                            scanner.connect(to: candidate)
                        } label: {
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(candidate.displayName)
                                    Text("\(candidate.rssi) dBm")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Image(systemName: "chevron.right")
                                    .foregroundStyle(.tertiary)
                            }
                        }
                    }
                }
            }
        case .connecting(let candidate), .reading(let candidate):
            Section { ProgressView("Reading \(candidate.displayName)…") }
        case .identified(let info, let candidate):
            Section("Selected device") {
                LabeledContent("Name", value: candidate.displayName)
                LabeledContent("Factory ID", value: info.deviceId)
                LabeledContent("Model", value: info.model)
                LabeledContent("Firmware", value: info.firmwareVersion)
                LabeledContent("Transport", value: info.transport)
                Button {
                    Task { await model.prepare(info: info) }
                } label: {
                    Label("Register and Provision", systemImage: "lock.open.rotation")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
            }
        case .unavailable(let availability):
            Section {
                ContentUnavailableView(
                    bluetoothUnavailableTitle(availability),
                    systemImage: "antenna.radiowaves.left.and.right.slash",
                    description: Text(bluetoothUnavailableMessage(availability))
                )
            }
        case .failed(let message):
            Section {
                Label(message, systemImage: "exclamationmark.triangle")
                Button("Scan Again") { scanner.startScanning() }
            }
        }
    }

    @ViewBuilder
    private var transferSections: some View {
        Section {
            ProgressView(value: scanner.provisioningState.progress)
            switch scanner.provisioningState {
            case .idle:
                Text("Preparing protected device credentials…")
            case .transferring(let completed, let total):
                Text("Sent \(completed.formatted(.byteCount(style: .file))) of \(total.formatted(.byteCount(style: .file)))")
                    .foregroundStyle(.secondary)
            case .succeeded:
                Label("Provisioning accepted", systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.green)
            case .failed:
                Button("Retry Transfer") {
                    if let payload = model.prepared?.payload {
                        scanner.provision(payload: payload)
                    }
                }
            }
        } header: {
            Text("Secure transfer")
        } footer: {
            Text("Keep the device nearby. Its assigned ID, compiled contract, CA chain, and private certificate key are transferred and verified before commit.")
        }
    }

    @ViewBuilder
    private var completionSections: some View {
        Section {
            ContentUnavailableView(
                "Device Provisioned",
                systemImage: "checkmark.seal.fill",
                description: Text("\(model.registeredDevice?.name ?? "The device") is registered and has everything needed to connect and publish telemetry.")
            )
        }
        if let device = model.registeredDevice {
            Section("Inventory") {
                LabeledContent("Device ID", value: device.id)
                LabeledContent("Blueprint", value: model.selectedBlueprint?.name ?? "—")
                LabeledContent("Fleet", value: model.fleets.first { $0.id == device.fleetId }?.name ?? "No fleet")
            }
        }
    }

    @ToolbarContentBuilder
    private var toolbarContent: some ToolbarContent {
        ToolbarItem(placement: .cancellationAction) {
            Button(model.isRollingBack ? "Removing…" : "Cancel") {
                Task {
                    guard await model.rollbackIfNeeded() else { return }
                    dismiss()
                }
            }
            .disabled(model.step == .registering || model.isRollingBack || model.step == .completed)
        }

        ToolbarItem(placement: .confirmationAction) {
            switch model.step {
            case .configure:
                Button("Find Device") { model.beginScanning() }
                    .disabled(!model.canContinue)
            case .scan:
                Button("Back") {
                    scanner.stopScanning()
                    model.step = .configure
                }
            case .completed:
                Button("Done") {
                    if let device = model.registeredDevice { onProvisioned(device) }
                    dismiss()
                }
            case .loading, .registering, .transfer:
                EmptyView()
            }
        }
    }

    private var blueprintSelection: Binding<String> {
        Binding(
            get: { model.selectedBlueprintId },
            set: { id in Task { await model.selectBlueprint(id) } }
        )
    }

    private var stepTitle: String {
        switch model.step {
        case .loading: "Getting Ready"
        case .configure: "Configure Inventory"
        case .scan: "Choose Nearby Hardware"
        case .registering: "Registering Device"
        case .transfer: "Sending Bootstrap Data"
        case .completed: "Ready"
        }
    }

    private var stepMessage: String {
        switch model.step {
        case .loading: "Loading the choices required by the server."
        case .configure: "Choose the name, blueprint, fleet, and transport bindings."
        case .scan: "Select an advertising device or bring the phone close to connect automatically."
        case .registering: "Compiling its contract and issuing one-time credentials."
        case .transfer: "Writing the verified provisioning package over Bluetooth."
        case .completed: "The hardware and inventory record are now linked."
        }
    }

    private var stepIcon: String {
        switch model.step {
        case .loading: "arrow.triangle.2.circlepath"
        case .configure: "slider.horizontal.3"
        case .scan: "dot.radiowaves.left.and.right"
        case .registering: "server.rack"
        case .transfer: "lock.shield"
        case .completed: "checkmark.seal.fill"
        }
    }

    private func bluetoothUnavailableTitle(_ availability: NearbyDeviceAvailability) -> String {
        switch availability {
        case .bluetoothOff: "Bluetooth Is Off"
        case .unauthorized: "Bluetooth Access Needed"
        case .unsupported: "Bluetooth Unavailable"
        }
    }

    private func bluetoothUnavailableMessage(_ availability: NearbyDeviceAvailability) -> String {
        switch availability {
        case .bluetoothOff: "Turn on Bluetooth, then return to this screen."
        case .unauthorized: "Allow Bluetooth access for Extrittio in Settings."
        case .unsupported: "This iPhone cannot scan for Bluetooth Low Energy devices."
        }
    }
}
