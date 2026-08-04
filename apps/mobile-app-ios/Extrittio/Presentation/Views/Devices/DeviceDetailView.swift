import SwiftUI

enum DeviceCategory: String, CaseIterable {
    case general = "General"
    case advanced = "Advanced"
}

enum GeneralSubTab: String, CaseIterable {
    case overview = "Overview"
    case telemetry = "Telemetry"
    case location = "Location"
    case logs = "Logs"
    case alerts = "Alerts"
    case ota = "OTA"

    var icon: String {
        switch self {
        case .overview: "list.bullet.rectangle"
        case .telemetry: "chart.line.uptrend.xyaxis"
        case .location: "location"
        case .logs: "doc.text.magnifyingglass"
        case .alerts: "bell"
        case .ota: "arrow.down.circle"
        }
    }

    var requiredPermissions: [PermissionKey] {
        switch self {
        case .overview: []
        case .telemetry, .location: [.telemetryRead]
        case .logs: [.logsRead]
        case .alerts: [.alertsRead]
        case .ota: [.firmwareRead, .shadowsRead]
        }
    }
}

enum AdvancedSubTab: String, CaseIterable {
    case details = "Details"
    case shadow = "Shadow"
    case commands = "Commands"
    case config = "Config"

    var icon: String {
        switch self {
        case .details: "info.circle"
        case .shadow: "doc.text"
        case .commands: "terminal"
        case .config: "gearshape"
        }
    }

    var requiredPermissions: [PermissionKey] {
        switch self {
        case .details: []
        case .shadow: [.shadowsRead]
        case .commands: [.commandsRead]
        case .config: []
        }
    }
}

struct DeviceDetailView: View {
    let deviceId: String
    @State private var viewModel: DeviceDetailViewModel
    @State private var selectedCategory: DeviceCategory = .general
    @State private var selectedGeneralTab: GeneralSubTab = .overview
    @State private var selectedAdvancedTab: AdvancedSubTab = .details
    @State private var showDeleteConfirmation = false
    @State private var showEditSheet = false
    @State private var isRestarting = false
    @State private var isDeleting = false
    @Environment(\.dismiss) private var dismiss
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    @Namespace private var tabAnimation

    let container: DependencyContainer

    init(deviceId: String, container: DependencyContainer) {
        self.deviceId = deviceId
        self.container = container
        self._viewModel = State(initialValue: container.makeDeviceDetailViewModel(deviceId: deviceId))
    }

    var body: some View {
        Group {
            switch viewModel.deviceState {
            case .loading:
                LoadingView("Loading device...")
            case .loaded(let device), .cached(let device, _):
                VStack(spacing: 0) {
                    if let cacheDate = viewModel.deviceState.cacheDate {
                        StalenessLabel(date: cacheDate)
                            .padding(.vertical, 4)
                    }
                    deviceHeader(device)
                    if !connectionMonitor.isOnline, can(.devicesManage) {
                        OfflineActionHint(message: "Device management actions are unavailable while offline.")
                            .padding(.horizontal)
                            .padding(.bottom, Spacing.sm)
                    }
                    categoryPicker
                    subTabPicker
                    tabContent
                }
            case .error(let error):
                EmptyStateView(
                    icon: "exclamationmark.triangle",
                    title: "Failed to Load",
                    message: error.localizedDescription
                )
            case .empty:
                EmptyStateView(
                    icon: "exclamationmark.triangle",
                    title: "Device Not Found",
                    message: "Could not load device details."
                )
            case .loadingMore(let device):
                VStack(spacing: 0) {
                    deviceHeader(device)
                    categoryPicker
                    subTabPicker
                    tabContent
                }
            }
        }
        .navigationTitle(viewModel.device?.name ?? "Device")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if can(.devicesManage) {
                ToolbarItem(placement: .primaryAction) {
                    Menu {
                        Button {
                            showEditSheet = true
                        } label: {
                            Label("Edit", systemImage: "pencil")
                        }
                        .disabled(!connectionMonitor.isOnline || isRestarting || isDeleting)
                        Button {
                            isRestarting = true
                            HapticEngine.shared.impact(.medium)
                            Task {
                                let success = await viewModel.restartDevice()
                                isRestarting = false
                                toastManager.show(success ? .success("Device restart command sent") : .error("Failed to restart device"))
                            }
                        } label: {
                            Label(isRestarting ? "Restarting" : "Restart", systemImage: isRestarting ? "hourglass" : "arrow.clockwise")
                        }
                        .disabled(!connectionMonitor.isOnline || isRestarting || isDeleting)
                        Button(role: .destructive) {
                            showDeleteConfirmation = true
                        } label: {
                            Label(isDeleting ? "Deleting" : "Delete", systemImage: isDeleting ? "hourglass" : "trash")
                        }
                        .disabled(!connectionMonitor.isOnline || isRestarting || isDeleting)
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
        }
        .confirmationDialog("Delete Device?", isPresented: $showDeleteConfirmation) {
            Button("Delete", role: .destructive) {
                isDeleting = true
                Task {
                    if await viewModel.deleteDevice() {
                        toastManager.show(.success("Device deleted"))
                        dismiss()
                    } else {
                        isDeleting = false
                        toastManager.show(.error("Failed to delete device"))
                    }
                }
            }
        } message: {
            Text("This action cannot be undone.")
        }
        .task { await viewModel.loadDevice() }
        .onAppear { ensureAccessibleSelection() }
        .sheet(isPresented: $showEditSheet) {
            if let device = viewModel.device {
                EditDeviceSheet(
                    device: device,
                    updateDeviceUseCase: container.makeUpdateDeviceUseCase(),
                    getDeviceTypesUseCase: container.makeGetDeviceTypesUseCase(),
                    getFleetsUseCase: container.makeGetFleetsUseCase()
                ) { updated in
                    viewModel.device = updated
                }
            }
        }
    }

    // MARK: - Header

    @ViewBuilder
    private func deviceHeader(_ device: Device) -> some View {
        HStack(alignment: .top, spacing: Spacing.md) {
            headerDeviceIllustration(device)
                .frame(width: 132, height: 112)

            headerInfo(device)
        }
        .padding()
        .glassCard()
        .padding(.horizontal)
        .slideIn()
    }

    private func headerDeviceIllustration(_ device: Device) -> some View {
        DeviceMockupView(
            deviceTypeName: device.deviceTypeName,
            status: device.status,
            size: .large
        )
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.thinMaterial, in: .rect(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(.quaternary, lineWidth: 1)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Device illustration")
    }

    @ViewBuilder
    private func headerInfo(_ device: Device) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            // Row 1: Name + status
            HStack {
                Text(device.name)
                    .font(.title3.weight(.semibold))
                    .lineLimit(1)
                Spacer()
                StatusBadge(status: device.status)
            }

            // Row 2: Type, fleet, firmware pill
            HStack(spacing: Spacing.sm) {
                if let typeName = device.deviceTypeName {
                    Text(typeName)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                if let fleetName = device.fleetName {
                    Text("·")
                        .foregroundStyle(.secondary)
                    Text(fleetName)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer()
                Text(device.firmwareVersionLabel)
                    .font(.caption.monospaced())
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, 3)
                    .background(.thinMaterial, in: .rect(cornerRadius: 6))
            }

            // Row 3: Last seen + uptime
            ViewThatFits(in: .horizontal) {
                HStack(spacing: Spacing.lg) {
                    headerTiming(device)
                }
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    headerTiming(device)
                }
            }
        }
    }

    @ViewBuilder
    private func headerTiming(_ device: Device) -> some View {
        if let lastSeen = device.lastSeen {
            Label(String.formattedTimestamp(lastSeen), systemImage: "clock")
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        Label(formatUptime(device.uptimeSeconds), systemImage: "timer")
            .font(.caption)
            .foregroundStyle(.secondary)
            .lineLimit(1)
    }

    // MARK: - Category Picker (Top Level)

    private var categoryPicker: some View {
        Picker("Category", selection: $selectedCategory) {
            ForEach(accessibleCategories, id: \.self) { category in
                Text(category.rawValue).tag(category)
            }
        }
        .pickerStyle(.segmented)
        .padding(.horizontal)
        .padding(.vertical, Spacing.sm)
        .onChange(of: selectedCategory) { _, _ in
            HapticEngine.shared.selection()
        }
    }

    // MARK: - Sub-tab Picker

    @ViewBuilder
    private var subTabPicker: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: Spacing.sm) {
                switch selectedCategory {
                case .general:
                    ForEach(accessibleGeneralTabs, id: \.self) { tab in
                        subTabButton(
                            tab.rawValue,
                            icon: tab.icon,
                            isSelected: selectedGeneralTab == tab,
                            matchId: tab.rawValue
                        ) {
                            withAnimation(AppAnimation.standard.animation) {
                                selectedGeneralTab = tab
                            }
                            HapticEngine.shared.selection()
                        }
                    }
                case .advanced:
                    ForEach(accessibleAdvancedTabs, id: \.self) { tab in
                        subTabButton(
                            tab.rawValue,
                            icon: tab.icon,
                            isSelected: selectedAdvancedTab == tab,
                            matchId: tab.rawValue
                        ) {
                            withAnimation(AppAnimation.standard.animation) {
                                selectedAdvancedTab = tab
                            }
                            HapticEngine.shared.selection()
                        }
                    }
                }
            }
            .padding(.horizontal)
        }
        .padding(.bottom, Spacing.sm)
    }

    private func subTabButton(_ label: String, icon: String, isSelected: Bool, matchId: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(label, systemImage: icon)
                .font(.subheadline)
                .fontWeight(isSelected ? .semibold : .regular)
                .padding(.horizontal, 14)
                .padding(.vertical, Spacing.sm)
        }
        .foregroundStyle(isSelected ? .white : .primary)
        .background {
            if isSelected {
                RoundedRectangle(cornerRadius: 12)
                    .fill(.tint)
                    .matchedGeometryEffect(id: "activeTab", in: tabAnimation)
            } else {
                RoundedRectangle(cornerRadius: 12)
                    .fill(.thinMaterial)
            }
        }
        .pressEffect()
    }

    // MARK: - Tab Content

    @ViewBuilder
    private var tabContent: some View {
        Group {
            switch selectedCategory {
            case .general:
                switch selectedGeneralTab {
                case .overview: DeviceOverviewTab(viewModel: viewModel)
                case .telemetry: DeviceTelemetryTab(viewModel: viewModel)
                case .location: DeviceLocationTab(viewModel: viewModel)
                case .logs: DeviceLogsTab(viewModel: viewModel)
                case .alerts: DeviceAlertsTab(viewModel: viewModel)
                case .ota: DeviceOTATab(viewModel: viewModel)
                }
            case .advanced:
                switch selectedAdvancedTab {
                case .details: DeviceDetailsTab(viewModel: viewModel)
                case .shadow: DeviceShadowTab(viewModel: viewModel)
                case .commands: DeviceCommandsTab(viewModel: viewModel)
                case .config: DeviceConfigTab(viewModel: viewModel)
                }
            }
        }
        .fadeTransition()
    }

    private var accessibleGeneralTabs: [GeneralSubTab] {
        GeneralSubTab.allCases.filter { hasRequired($0.requiredPermissions) }
    }

    private var accessibleAdvancedTabs: [AdvancedSubTab] {
        AdvancedSubTab.allCases.filter { hasRequired($0.requiredPermissions) }
    }

    private var accessibleCategories: [DeviceCategory] {
        DeviceCategory.allCases.filter { category in
            switch category {
            case .general: !accessibleGeneralTabs.isEmpty
            case .advanced: !accessibleAdvancedTabs.isEmpty
            }
        }
    }

    private func can(_ permission: PermissionKey) -> Bool {
        authViewModel.currentUser?.hasPermission(permission) == true
    }

    private func hasRequired(_ permissions: [PermissionKey]) -> Bool {
        permissions.allSatisfy(can)
    }

    private func ensureAccessibleSelection() {
        if !accessibleCategories.contains(selectedCategory) {
            selectedCategory = accessibleCategories.first ?? .general
        }
        if !accessibleGeneralTabs.contains(selectedGeneralTab) {
            selectedGeneralTab = accessibleGeneralTabs.first ?? .overview
        }
        if !accessibleAdvancedTabs.contains(selectedAdvancedTab) {
            selectedAdvancedTab = accessibleAdvancedTabs.first ?? .details
        }
    }

    // MARK: - Helpers

    private func formatUptime(_ seconds: Int) -> String {
        let days = seconds / 86400
        let hours = (seconds % 86400) / 3600
        let mins = (seconds % 3600) / 60
        if days > 0 { return "\(days)d \(hours)h \(mins)m" }
        if hours > 0 { return "\(hours)h \(mins)m" }
        return "\(mins)m"
    }
}
