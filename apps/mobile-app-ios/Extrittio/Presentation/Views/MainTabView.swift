import SwiftUI

struct MainTabView: View {
    @Environment(AuthViewModel.self) private var authViewModel
    let container: DependencyContainer
    @State private var selectedTab: TabItem = .dashboard
    @State private var dashboardVM: DashboardViewModel?
    @State private var deviceListVM: DeviceListViewModel?
    @State private var mapVM: MapViewModel?
    @State private var alertsVM: AlertsViewModel?
    @State private var rulesVM: RulesViewModel?
    var body: some View {
        TabView(selection: $selectedTab) {
            if canAccess(.dashboard) {
                Tab(TabItem.dashboard.rawValue, systemImage: TabItem.dashboard.icon, value: .dashboard) {
                    if let vm = dashboardVM {
                        DashboardView(
                            viewModel: vm,
                            openDevices: canAccess(.devices) ? { selectedTab = .devices } : nil,
                            openAlerts: canAccess(.alerts) ? { selectedTab = .alerts } : nil
                        )
                    }
                }
            }

            if canAccess(.map) {
                Tab(TabItem.map.rawValue, systemImage: TabItem.map.icon, value: .map) {
                    if let vm = mapVM {
                        MapView(viewModel: vm, container: container)
                    }
                }
            }

            if canAccess(.devices) {
                Tab(TabItem.devices.rawValue, systemImage: TabItem.devices.icon, value: .devices) {
                    if let vm = deviceListVM {
                        DeviceListView(viewModel: vm, container: container)
                    }
                }
            }

            if canAccess(.alerts) {
                Tab(TabItem.alerts.rawValue, systemImage: TabItem.alerts.icon, value: .alerts) {
                    if let vm = alertsVM {
                        AlertsView(viewModel: vm)
                    }
                }
            }

            if canAccess(.rules) {
                Tab(TabItem.rules.rawValue, systemImage: TabItem.rules.icon, value: .rules) {
                    if let vm = rulesVM {
                        RulesView(viewModel: vm)
                    }
                }
            }

            Tab(TabItem.settings.rawValue, systemImage: TabItem.settings.icon, value: .settings) {
                SettingsView(container: container)
            }
        }
        .tabViewStyle(.sidebarAdaptable)
        .task {
            dashboardVM = dashboardVM ?? container.makeDashboardViewModel()
            deviceListVM = deviceListVM ?? container.makeDeviceListViewModel()
            mapVM = mapVM ?? container.makeMapViewModel()
            alertsVM = alertsVM ?? container.makeAlertsViewModel()
            rulesVM = rulesVM ?? container.makeRulesViewModel()
        }
        .onAppear { ensureAccessibleSelection() }
        .onChange(of: authViewModel.currentUser?.permissionVersion) { _, _ in ensureAccessibleSelection() }
    }

    private func canAccess(_ tab: TabItem) -> Bool {
        switch tab {
        case .map:
            has(.devicesRead) && has(.zonesRead)
        case .devices:
            has(.devicesRead)
        case .alerts:
            has(.alertsRead)
        case .rules:
            has(.rulesRead)
        case .dashboard:
            has(.serverMetricsRead)
        case .settings:
            true
        }
    }

    private func has(_ permission: PermissionKey) -> Bool {
        authViewModel.currentUser?.hasPermission(permission) == true
    }

    private func ensureAccessibleSelection() {
        guard !canAccess(selectedTab) else { return }
        selectedTab = TabItem.allCases.first(where: canAccess) ?? .settings
    }
}

enum TabItem: String, CaseIterable {
    case dashboard = "Dashboard"
    case map = "Map"
    case devices = "Devices"
    case alerts = "Alerts"
    case rules = "Rules"
    case settings = "Settings"

    var icon: String {
        switch self {
        case .dashboard: "chart.line.uptrend.xyaxis"
        case .map: "map"
        case .devices: "sensor.tag.radiowaves.forward"
        case .alerts: "bell.badge"
        case .rules: "gearshape.2"
        case .settings: "gear"
        }
    }
}
