import SwiftUI

struct DashboardView: View {
    let viewModel: DashboardViewModel
    var openDevices: (() -> Void)?
    var openAlerts: (() -> Void)?
    @State private var selectedTab: DashboardTab = .devices

    enum DashboardTab: String, CaseIterable {
        case devices = "Devices"
        case infrastructure = "Infrastructure"
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker("Dashboard", selection: $selectedTab) {
                    ForEach(DashboardTab.allCases, id: \.self) { tab in
                        Text(tab.rawValue).tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal)
                .padding(.vertical, 8)
                .onChange(of: selectedTab) {
                    HapticEngine.shared.selection()
                }

                dashboardContent
            }
            .overlay(alignment: .top) {
                if viewModel.showRefreshBar {
                    refreshBar
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
            }
            .animation(AppAnimation.quick.animation, value: viewModel.showRefreshBar)
            .navigationTitle("Dashboard")
            .refreshable {
                await viewModel.load()
            }
            .task {
                await viewModel.load()
            }
            .task {
                await viewModel.startAutoRefresh()
            }
        }
    }

    // MARK: - Content

    @ViewBuilder
    private var dashboardContent: some View {
        let allLoading = viewModel.statsState.isLoading
            && viewModel.metricsState.isLoading

        let allError = viewModel.statsState.errorMessage != nil
            && viewModel.statsState.data == nil
            && viewModel.metricsState.errorMessage != nil
            && viewModel.metricsState.data == nil

        if allLoading {
            DashboardSkeletonView()
                .frame(maxHeight: .infinity, alignment: .top)
        } else if allError {
            ContentUnavailableView {
                Label("Error", systemImage: "exclamationmark.triangle")
            } description: {
                Text(viewModel.statsState.errorMessage ?? "Failed to load dashboard data")
            } actions: {
                Button("Retry") {
                    Task { await viewModel.load() }
                }
            }
            .frame(maxHeight: .infinity)
        } else {
            VStack(spacing: 0) {
                if let cacheDate = dashboardCacheDate {
                    StalenessLabel(date: cacheDate)
                        .padding(.vertical, 4)
                }
                switch selectedTab {
                case .infrastructure:
                    InfrastructureTabView(
                        metrics: viewModel.metricsState.data,
                        history: viewModel.metricsHistoryState.data,
                        metricsError: viewModel.metricsState.errorMessage
                    )
                case .devices:
                    DevicesTabView(
                        stats: viewModel.statsState.data,
                        alertSummary: viewModel.alertSummaryState.data,
                        openDevices: openDevices,
                        openAlerts: openAlerts
                    )
                }
            }
        }
    }

    private var dashboardCacheDate: Date? {
        viewModel.statsState.cacheDate ?? viewModel.metricsState.cacheDate
    }

    // MARK: - Refresh Bar

    private var refreshBar: some View {
        LinearGradient(
            colors: [.blue.opacity(0.8), .cyan.opacity(0.6)],
            startPoint: .leading,
            endPoint: .trailing
        )
        .frame(height: 3)
    }
}
