import SwiftUI

struct EditDeviceSheet: View {
    let device: Device
    let updateDeviceUseCase: UpdateDeviceUseCase
    let getDeviceTypesUseCase: GetDeviceTypesUseCase
    let getFleetsUseCase: GetFleetsUseCase
    @Environment(\.dismiss) private var dismiss
    @State private var name: String
    @State private var firmware: String
    @State private var selectedDeviceTypeId: Int
    @State private var selectedFleetId: Int?
    @State private var deviceTypes: [DeviceType] = []
    @State private var fleets: [Fleet] = []
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var errorMessage: String?

    var onUpdated: ((Device) -> Void)?

    private var selectedDeviceType: DeviceType? {
        deviceTypes.first { $0.id == selectedDeviceTypeId }
    }

    init(device: Device, updateDeviceUseCase: UpdateDeviceUseCase, getDeviceTypesUseCase: GetDeviceTypesUseCase, getFleetsUseCase: GetFleetsUseCase, onUpdated: ((Device) -> Void)? = nil) {
        self.device = device
        self.updateDeviceUseCase = updateDeviceUseCase
        self.getDeviceTypesUseCase = getDeviceTypesUseCase
        self.getFleetsUseCase = getFleetsUseCase
        self.onUpdated = onUpdated
        self._name = State(initialValue: device.name)
        self._firmware = State(initialValue: device.firmware)
        self._selectedDeviceTypeId = State(initialValue: device.deviceTypeId)
        self._selectedFleetId = State(initialValue: device.fleetId)
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 20) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Device Info")
                            .font(.headline)
                        TextField("Device Name", text: $name)
                            .autocorrectionDisabled()
                            .padding()
                            .glassCard()
                        TextField("Firmware Version", text: $firmware)
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                            .padding()
                            .glassCard()
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Text("Device Type")
                            .font(.headline)
                        if isLoading {
                            ProgressView()
                                .frame(maxWidth: .infinity)
                                .padding()
                                .glassCard()
                        } else {
                            Picker("Type", selection: $selectedDeviceTypeId) {
                                ForEach(deviceTypes) { dt in
                                    Text(dt.name).tag(dt.id)
                                }
                            }
                            .padding()
                            .glassCard()

                            if let selectedDeviceType {
                                deviceTypeMockup(selectedDeviceType)
                            }
                        }
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Text("Fleet (Optional)")
                            .font(.headline)
                        Picker("Fleet", selection: $selectedFleetId) {
                            Text("None").tag(nil as Int?)
                            ForEach(fleets) { fleet in
                                Text(fleet.name).tag(fleet.id as Int?)
                            }
                        }
                        .padding()
                        .glassCard()
                    }

                    if let error = errorMessage {
                        HStack {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(.red)
                            Text(error)
                                .font(.callout)
                                .foregroundStyle(.red)
                        }
                        .padding()
                        .frame(maxWidth: .infinity)
                        .glassCard()
                    }
                }
                .padding()
            }
            .navigationTitle("Edit Device")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { saveDevice() }
                        .disabled(name.isEmpty || isSaving)
                        .bold()
                }
            }
            .task { await loadSupportingData() }
        }
    }

    private func loadSupportingData() async {
        isLoading = true
        do {
            async let dtResponse = getDeviceTypesUseCase.execute()
            async let fleetResponse = getFleetsUseCase.execute()
            deviceTypes = try await dtResponse
            fleets = try await fleetResponse
        } catch {
            errorMessage = error.localizedDescription
        }
        isLoading = false
    }

    private func saveDevice() {
        isSaving = true
        errorMessage = nil
        Task {
            let request = UpdateDeviceRequest(
                name: name,
                deviceTypeId: selectedDeviceTypeId,
                fleetId: selectedFleetId,
                firmware: firmware
            )
            do {
                let updated = try await updateDeviceUseCase.execute(id: device.id, request)
                onUpdated?(updated)
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
            isSaving = false
        }
    }

    private func deviceTypeMockup(_ type: DeviceType) -> some View {
        HStack(spacing: Spacing.md) {
            DeviceMockupView(
                deviceTypeName: type.name,
                icon: type.icon,
                colorHex: type.colorHex,
                size: .medium
            )
            .frame(width: 82, height: 70)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(type.name)
                    .font(.subheadline.weight(.semibold))
                if let icon = type.icon, !icon.isEmpty {
                    Text(icon)
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            Spacer()
        }
        .padding()
        .glassCard()
    }
}
