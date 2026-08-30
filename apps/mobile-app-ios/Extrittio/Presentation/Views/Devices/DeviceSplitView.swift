import SwiftUI

struct DeviceSplitView: View {
    @State private var selectedDeviceID: String?
    @Environment(ToastManager.self) private var toastManager
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    let viewModel: DeviceListViewModel
    let container: DependencyContainer
    @State private var restartingDeviceId: String?
    @State private var deletingDeviceId: String?
    @State private var isSelectingDevices = false
    @State private var selectedDeviceIds = Set<String>()
    @State private var pendingBulkAction: DeviceBulkAction?
    @State private var bulkActionInProgress: DeviceBulkAction?
    let showNearbyDevicePanel: () -> Void
    let showProvisionDevicePanel: () -> Void

    private var canManageDevices: Bool {
        authViewModel.currentUser?.hasPermission(.devicesManage) == true
    }

    private var canProvisionDevices: Bool {
        authViewModel.currentUser?.hasRequiredPermissions([
            .devicesManage, .deviceBlueprintsRead, .fleetsRead
        ]) == true
    }

    private var selectedFleetName: String? {
        guard let selectedFleetId = viewModel.selectedFleetId else { return nil }
        return viewModel.fleets.first { $0.id == selectedFleetId }?.name
    }

    private var hasActiveFilters: Bool {
        viewModel.statusFilter != nil || viewModel.selectedFleetId != nil || !viewModel.searchText.isEmpty
    }

    var body: some View {
        HStack(spacing: 0) {
            deviceListPanel
                .frame(minWidth: 280, idealWidth: 320, maxWidth: 360)
            Divider()
            detailPanel
                .frame(maxWidth: .infinity)
        }
        .confirmationDialog(
            pendingBulkAction?.confirmationTitle ?? "Apply Action?",
            isPresented: Binding(
                get: { pendingBulkAction != nil },
                set: { if !$0 { pendingBulkAction = nil } }
            ),
            titleVisibility: .visible
        ) {
            if let pendingBulkAction {
                Button(pendingBulkAction.confirmationButtonTitle, role: pendingBulkAction == .delete ? .destructive : nil) {
                    performBulkAction(pendingBulkAction)
                }
            }
            Button("Cancel", role: .cancel) {
                pendingBulkAction = nil
            }
        } message: {
            if let pendingBulkAction {
                Text(pendingBulkAction.confirmationMessage(count: selectedDeviceIds.count))
            }
        }
    }

    private var deviceListPanel: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Devices")
                    .font(.title2.bold())
                Spacer()
                if canManageDevices, viewModel.devicesState.data?.isEmpty == false {
                    Button {
                        toggleSelectionMode()
                    } label: {
                        Image(systemName: isSelectingDevices ? "checkmark.circle.fill" : "checkmark.circle")
                    }
                    .disabled(deviceActionInProgress)
                    .accessibilityLabel(isSelectingDevices ? "Done Selecting" : "Select Devices")
                }
                if canProvisionDevices, !isSelectingDevices {
                    Button {
                        showProvisionDevicePanel()
                    } label: {
                        Image(systemName: "sensor.tag.radiowaves.forward")
                    }
                    .disabled(!connectionMonitor.isOnline)
                    .accessibilityLabel("Provision Device")
                }
                if !isSelectingDevices {
                    Button(action: showNearbyDevicePanel) {
                        Image(systemName: "dot.radiowaves.left.and.right")
                    }
                    .accessibilityLabel("Tap into a device")
                    Menu {
                        Button("All") {
                            viewModel.statusFilter = nil
                            HapticEngine.shared.selection()
                            Task { await viewModel.load() }
                        }
                        Button("Online") {
                            viewModel.statusFilter = "online"
                            HapticEngine.shared.selection()
                            Task { await viewModel.load() }
                        }
                        Button("Offline") {
                            viewModel.statusFilter = "offline"
                            HapticEngine.shared.selection()
                            Task { await viewModel.load() }
                        }
                    } label: {
                        Image(systemName: "line.3.horizontal.decrease.circle")
                    }
                    if !viewModel.fleets.isEmpty {
                        Menu {
                            Button("All Fleets") {
                                viewModel.selectedFleetId = nil
                                HapticEngine.shared.selection()
                                Task { await viewModel.load() }
                            }
                            ForEach(viewModel.fleets) { fleet in
                                Button(fleet.name) {
                                    viewModel.selectedFleetId = fleet.id
                                    HapticEngine.shared.selection()
                                    Task { await viewModel.load() }
                                }
                            }
                        } label: {
                            Image(systemName: "folder")
                        }
                    }
                }
            }
            .padding()

            listSummary
                .padding(.horizontal)
                .padding(.bottom, Spacing.sm)

            Group {
                switch viewModel.devicesState {
                case .loading:
                    DeviceListSkeletonView()
                        .frame(maxHeight: .infinity, alignment: .top)
                        .padding(.top, Spacing.sm)
                case .empty:
                    emptyDevicesView
                case .error(let error):
                    ErrorBanner(message: error.localizedDescription) {
                        Task { await viewModel.load() }
                    }
                case .loaded(let devices), .cached(let devices, _), .loadingMore(let devices):
                    List(devices) { device in
                        if isSelectingDevices {
                            Button {
                                toggleSelection(for: device.id)
                            } label: {
                                SelectableDeviceRow(
                                    device: device,
                                    isSelected: selectedDeviceIds.contains(device.id),
                                    isDimmed: restartingDeviceId == device.id || deletingDeviceId == device.id || bulkActionInProgress != nil
                                )
                            }
                            .buttonStyle(.plain)
                            .onAppear {
                                if device.id == devices.last?.id {
                                    Task { await viewModel.loadMore() }
                                }
                            }
                        } else {
                            Button {
                                selectedDeviceID = device.id
                            } label: {
                                DeviceRowView(device: device)
                                    .overlay {
                                        if selectedDeviceID == device.id {
                                            RoundedRectangle(cornerRadius: 8)
                                                .stroke(Color.accentColor.opacity(0.35), lineWidth: 1)
                                        }
                                    }
                            }
                            .buttonStyle(.plain)
                            .opacity(restartingDeviceId == device.id || deletingDeviceId == device.id ? 0.6 : 1)
                            .contextMenu {
                                if canManageDevices {
                                    Button {
                                        restartDevice(device.id)
                                    } label: {
                                        Label(restartingDeviceId == device.id ? "Restarting" : "Restart", systemImage: restartingDeviceId == device.id ? "hourglass" : "arrow.clockwise")
                                    }
                                    .disabled(!connectionMonitor.isOnline || deviceActionInProgress)
                                    Button(role: .destructive) {
                                        deleteDevice(device.id)
                                    } label: {
                                        Label(deletingDeviceId == device.id ? "Deleting" : "Delete", systemImage: deletingDeviceId == device.id ? "hourglass" : "trash")
                                    }
                                    .disabled(!connectionMonitor.isOnline || deviceActionInProgress)
                                }
                            }
                            .onAppear {
                                if device.id == devices.last?.id {
                                    Task { await viewModel.loadMore() }
                                }
                            }
                        }
                    }
                    .listStyle(.plain)
                    .safeAreaInset(edge: .top) {
                        if let cacheDate = viewModel.devicesState.cacheDate {
                            StalenessLabel(date: cacheDate)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal)
                                .padding(.vertical, Spacing.xs)
                        }
                    }
                    .safeAreaInset(edge: .bottom) {
                        if isSelectingDevices {
                            bulkActionBar(devices: devices)
                        } else if !connectionMonitor.isOnline, canManageDevices {
                            OfflineActionHint(message: "Device changes are unavailable while offline.")
                                .padding(.horizontal)
                                .padding(.bottom, Spacing.xs)
                        }
                    }
                    .overlay(alignment: .bottom) {
                        if viewModel.devicesState.isLoadingMore {
                            ProgressView()
                                .padding()
                        }
                    }
                }
            }
            .searchable(text: Binding(get: { viewModel.searchText }, set: { viewModel.searchText = $0 }), prompt: "Search devices")
            .onChange(of: viewModel.searchText) { _, _ in
                HapticEngine.shared.selection()
                viewModel.debouncedSearch()
            }
            .onChange(of: viewModel.devicesState.data?.map(\.id) ?? []) { _, visibleIds in
                selectedDeviceIds.formIntersection(Set(visibleIds))
            }
            .refreshable { await viewModel.load() }
            .task { await viewModel.load() }
        }
    }

    private var listSummary: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text("\(viewModel.totalDevices) devices")
                        .font(.subheadline.weight(.semibold))
                    if let showing = viewModel.devicesState.data?.count {
                        Text("Showing \(showing)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Spacer()

                if hasActiveFilters {
                    Button("Clear") {
                        clearFilters()
                    }
                    .font(.caption.weight(.semibold))
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }

            if hasActiveFilters {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: Spacing.sm) {
                        if let statusFilter = viewModel.statusFilter {
                            filterChip("Status: \(statusFilter.capitalized)", systemImage: "circle.fill", color: statusFilter == "online" ? .green : .red) {
                                viewModel.statusFilter = nil
                                HapticEngine.shared.selection()
                                Task { await viewModel.load() }
                            }
                        }

                        if let selectedFleetName {
                            filterChip(selectedFleetName, systemImage: "folder", color: .blue) {
                                viewModel.selectedFleetId = nil
                                HapticEngine.shared.selection()
                                Task { await viewModel.load() }
                            }
                        }

                        if !viewModel.searchText.isEmpty {
                            filterChip("Search: \(viewModel.searchText)", systemImage: "magnifyingglass", color: .purple) {
                                viewModel.searchText = ""
                                HapticEngine.shared.selection()
                                Task { await viewModel.load() }
                            }
                        }
                    }
                }
            }
        }
    }

    private func filterChip(_ label: String, systemImage: String, color: Color, clear: @escaping () -> Void) -> some View {
        Button(action: clear) {
            HStack(spacing: Spacing.xs) {
                Image(systemName: systemImage)
                    .font(.caption2)
                    .foregroundStyle(color)
                Text(label)
                    .lineLimit(1)
                Image(systemName: "xmark.circle.fill")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .font(.caption.weight(.medium))
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, Spacing.xs + 1)
            .background(color.opacity(0.10), in: .capsule)
            .overlay(
                Capsule()
                    .stroke(color.opacity(0.20), lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }

    private func clearFilters() {
        viewModel.statusFilter = nil
        viewModel.selectedFleetId = nil
        viewModel.searchText = ""
        HapticEngine.shared.selection()
        Task { await viewModel.load() }
    }

    private var emptyDevicesView: some View {
        EmptyStateView(
            icon: "sensor.tag.radiowaves.forward",
            title: "No Devices",
            message: hasActiveFilters ? "No devices match the selected filters." : "No devices have been added yet.",
            actionTitle: hasActiveFilters ? "Clear Filters" : (canProvisionDevices ? "Provision Device" : nil),
            actionSystemImage: hasActiveFilters ? "xmark.circle" : "plus",
            isActionDisabled: !hasActiveFilters && !connectionMonitor.isOnline
        ) {
            if hasActiveFilters {
                clearFilters()
            } else {
                showProvisionDevicePanel()
            }
        }
    }

    private var deviceActionInProgress: Bool {
        restartingDeviceId != nil || deletingDeviceId != nil || bulkActionInProgress != nil
    }

    private func restartDevice(_ id: String) {
        restartingDeviceId = id
        Task {
            let success = await viewModel.restartDevice(id: id)
            restartingDeviceId = nil
            toastManager.show(success ? .success("Device restarting") : .error("Restart failed"))
        }
    }

    private func deleteDevice(_ id: String) {
        deletingDeviceId = id
        Task {
            let success = await viewModel.deleteDevice(id: id)
            deletingDeviceId = nil
            selectedDeviceIds.remove(id)
            if success, selectedDeviceID == id {
                selectedDeviceID = nil
            }
            toastManager.show(success ? .success("Device deleted") : .error("Delete failed"))
        }
    }

    private func toggleSelectionMode() {
        if isSelectingDevices {
            endSelectionMode()
        } else {
            isSelectingDevices = true
            HapticEngine.shared.selection()
        }
    }

    private func endSelectionMode() {
        isSelectingDevices = false
        selectedDeviceIds.removeAll()
        pendingBulkAction = nil
        HapticEngine.shared.selection()
    }

    private func toggleSelection(for deviceId: String) {
        if selectedDeviceIds.contains(deviceId) {
            selectedDeviceIds.remove(deviceId)
        } else {
            selectedDeviceIds.insert(deviceId)
        }
        HapticEngine.shared.selection()
    }

    private func bulkActionBar(devices: [Device]) -> some View {
        DeviceBulkActionBar(
            selectedCount: selectedDeviceIds.count,
            visibleCount: devices.count,
            allVisibleSelected: allVisibleDevicesSelected(devices),
            isOnline: connectionMonitor.isOnline,
            isWorking: bulkActionInProgress != nil,
            toggleVisibleSelection: { toggleVisibleSelection(devices) },
            restartSelected: { requestBulkAction(.restart) },
            deleteSelected: { requestBulkAction(.delete) }
        )
    }

    private func allVisibleDevicesSelected(_ devices: [Device]) -> Bool {
        !devices.isEmpty && devices.allSatisfy { selectedDeviceIds.contains($0.id) }
    }

    private func toggleVisibleSelection(_ devices: [Device]) {
        let visibleIds = Set(devices.map(\.id))
        if allVisibleDevicesSelected(devices) {
            selectedDeviceIds.subtract(visibleIds)
        } else {
            selectedDeviceIds.formUnion(visibleIds)
        }
        HapticEngine.shared.selection()
    }

    private func requestBulkAction(_ action: DeviceBulkAction) {
        guard !selectedDeviceIds.isEmpty else { return }
        pendingBulkAction = action
    }

    private func performBulkAction(_ action: DeviceBulkAction) {
        let ids = selectedDeviceIds
        let detailDeviceID = selectedDeviceID
        guard !ids.isEmpty else {
            pendingBulkAction = nil
            return
        }

        pendingBulkAction = nil
        bulkActionInProgress = action
        Task {
            let result: DeviceBulkActionResult
            switch action {
            case .restart:
                result = await viewModel.restartDevices(ids: ids)
            case .delete:
                result = await viewModel.deleteDevices(ids: ids)
                if let detailDeviceID, result.succeededIds.contains(detailDeviceID) {
                    selectedDeviceID = nil
                }
            }

            bulkActionInProgress = nil
            selectedDeviceIds = result.failedIds
            if selectedDeviceIds.isEmpty {
                isSelectingDevices = false
            }
            toastManager.show(action.toast(for: result))
        }
    }

    @ViewBuilder
    private var detailPanel: some View {
        if let deviceId = selectedDeviceID {
            NavigationStack {
                DeviceDetailView(deviceId: deviceId, container: container)
            }
            .id(deviceId)
        } else {
            ContentUnavailableView(
                "Select a Device",
                systemImage: "sensor.tag.radiowaves.forward",
                description: Text("Choose a device from the list to view its details.")
            )
        }
    }
}
