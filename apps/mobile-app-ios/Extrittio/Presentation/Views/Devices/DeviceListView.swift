import SwiftUI

struct DeviceListView: View {
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(ToastManager.self) private var toastManager
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @Environment(AppNavigationRouter.self) private var navigationRouter
    let viewModel: DeviceListViewModel
    let container: DependencyContainer
    @State private var restartingDeviceId: String?
    @State private var deletingDeviceId: String?
    @State private var isSelectingDevices = false
    @State private var selectedDeviceIds = Set<String>()
    @State private var pendingBulkAction: DeviceBulkAction?
    @State private var bulkActionInProgress: DeviceBulkAction?
    @State private var presentedPanel: DeviceListPanel?

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
        Group {
            if sizeClass == .regular {
                DeviceSplitView(
                    viewModel: viewModel,
                    container: container,
                    showNearbyDevicePanel: presentNearbyDevicePanel,
                    showProvisionDevicePanel: presentProvisionDevicePanel
                )
            } else {
                compactLayout
            }
        }
        .sheet(item: $presentedPanel) { panel in
            switch panel {
            case .nearbyDevice:
                NearbyDevicePanel(scanner: container.makeNearbyDeviceScanner())
                    .id(panel.id)
            case .provisionDevice:
                ProvisionDeviceSheet(
                    model: container.makeProvisionDeviceViewModel(),
                    scanner: container.makeProvisioningDeviceScanner()
                ) { device in
                    viewModel.insertDevice(device)
                }
                .id(panel.id)
            }
        }
        .onAppear { handleNavigationRequest() }
        .onChange(of: navigationRouter.request) { _, _ in handleNavigationRequest() }
    }

    private var compactLayout: some View {
        NavigationStack {
            Group {
                switch viewModel.devicesState {
                case .loading:
                    DeviceListSkeletonView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                        .padding(.top, Spacing.lg)
                case .empty:
                    emptyDevicesView
                case .error(let error):
                    ErrorBanner(message: error.localizedDescription) {
                        Task { await viewModel.load() }
                    }
                    .padding(.top, Spacing.lg)
                case .loaded(let devices), .cached(let devices, _), .loadingMore(let devices):
                    deviceList(devices: devices)
                }
            }
            .navigationTitle("Devices")
            .searchable(text: Bindable(viewModel).searchText, prompt: "Search devices")
            .onChange(of: viewModel.searchText) { _, _ in
                HapticEngine.shared.selection()
                viewModel.debouncedSearch()
            }
            .onChange(of: viewModel.devicesState.data?.map(\.id) ?? []) { _, visibleIds in
                selectedDeviceIds.formIntersection(Set(visibleIds))
            }
            .toolbar {
                if canManageDevices, viewModel.devicesState.data?.isEmpty == false {
                    ToolbarItem(placement: .topBarLeading) {
                        Button(isSelectingDevices ? "Done" : "Select") {
                            toggleSelectionMode()
                        }
                        .disabled(deviceActionInProgress)
                    }
                }
                if canProvisionDevices, !isSelectingDevices {
                    ToolbarItem(placement: .primaryAction) {
                        Button {
                            presentProvisionDevicePanel()
                        } label: {
                            Label("Provision Device", systemImage: "sensor.tag.radiowaves.forward")
                        }
                        .disabled(!connectionMonitor.isOnline)
                    }
                }
                if !isSelectingDevices {
                    ToolbarItem(placement: .primaryAction) {
                        Button {
                            presentNearbyDevicePanel()
                        } label: {
                            Label("Tap into a device", systemImage: "dot.radiowaves.left.and.right")
                        }
                    }
                    ToolbarItem(placement: .primaryAction) {
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
                            Label(viewModel.statusFilter?.capitalized ?? "Status", systemImage: "line.3.horizontal.decrease.circle")
                        }
                    }
                    ToolbarItem(placement: .primaryAction) {
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
                                Label(selectedFleetName ?? "Fleet", systemImage: "folder")
                            }
                        }
                    }
                }
            }
            .refreshable { await viewModel.load() }
            .task { await viewModel.load() }
            .navigationDestination(for: String.self) { deviceId in
                DeviceDetailView(deviceId: deviceId, container: container)
            }
            .safeAreaInset(edge: .bottom) {
                if isSelectingDevices, let devices = viewModel.devicesState.data {
                    bulkActionBar(devices: devices)
                }
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
    }

    private func deviceList(devices: [Device]) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Spacing.md) {
                listHeader(showing: devices.count)

                if let cacheDate = viewModel.devicesState.cacheDate {
                    StalenessLabel(date: cacheDate)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                if !connectionMonitor.isOnline, canManageDevices {
                    OfflineActionHint(message: "Device changes are unavailable while offline.")
                }
                ForEach(Array(devices.enumerated()), id: \.element.id) { index, device in
                    if isSelectingDevices {
                        Button {
                            toggleSelection(for: device.id)
                        } label: {
                            SelectableDeviceRow(
                                device: device,
                                isSelected: selectedDeviceIds.contains(device.id),
                                isDimmed: deletingDeviceId == device.id || restartingDeviceId == device.id || bulkActionInProgress != nil
                            )
                            .pressEffect()
                        }
                        .buttonStyle(.plain)
                        .slideIn(delay: Double(index) * 0.04)
                        .onAppear {
                            if device.id == devices.last?.id {
                                Task { await viewModel.loadMore() }
                            }
                        }
                    } else {
                        NavigationLink(value: device.id) {
                            DeviceRowView(device: device)
                                .pressEffect()
                        }
                        .buttonStyle(.plain)
                        .slideIn(delay: Double(index) * 0.04)
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
                        .opacity(deletingDeviceId == device.id || restartingDeviceId == device.id ? 0.6 : 1)
                        .onAppear {
                            if device.id == devices.last?.id {
                                Task { await viewModel.loadMore() }
                            }
                        }
                    }
                }

                if viewModel.devicesState.isLoadingMore {
                    ProgressView()
                        .padding()
                }
            }
            .padding()
        }
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
                presentProvisionDevicePanel()
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
            }

            bulkActionInProgress = nil
            selectedDeviceIds = result.failedIds
            if selectedDeviceIds.isEmpty {
                isSelectingDevices = false
            }
            toastManager.show(action.toast(for: result))
        }
    }

    private func listHeader(showing count: Int) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text("\(viewModel.totalDevices) devices")
                        .font(.subheadline.weight(.semibold))
                    Text("Showing \(count)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
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
                activeFilterChips
            }
        }
    }

    private var activeFilterChips: some View {
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

    private func handleNavigationRequest() {
        guard let request = navigationRouter.request else { return }

        switch request.destination {
        case .pairNearbyDevice:
            presentNearbyDevicePanel()
            navigationRouter.consume(request)
        }
    }

    private func presentNearbyDevicePanel() {
        presentedPanel = .nearbyDevice(id: UUID())
    }

    private func presentProvisionDevicePanel() {
        presentedPanel = .provisionDevice(id: UUID())
    }
}

private enum DeviceListPanel: Identifiable {
    case nearbyDevice(id: UUID)
    case provisionDevice(id: UUID)

    var id: UUID {
        switch self {
        case .nearbyDevice(let id), .provisionDevice(let id): id
        }
    }
}
