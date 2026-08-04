import SwiftUI

@main
struct ExtrittioApp: App {
    @State private var container = DependencyContainer()
    @State private var authViewModel: AuthViewModel?
    @State private var toastManager = ToastManager()

    var body: some Scene {
        WindowGroup {
            Group {
                if let authVM = authViewModel {
                    if authVM.isAuthenticated {
                        MainTabView(container: container)
                    } else {
                        LoginView()
                    }
                } else {
                    ProgressView()
                }
            }
            .overlay(alignment: .top) {
                VStack(spacing: 0) {
                    OfflineBanner()
                    if let toast = toastManager.currentToast {
                        ToastView(toast: toast) { toastManager.dismiss() }
                            .padding(.top, Spacing.xl)
                            .animation(AppAnimation.standard.animation, value: toastManager.currentToast)
                    }
                }
            }
            .task {
                if authViewModel == nil {
                    let authVM = container.makeAuthViewModel()
                    authViewModel = authVM
                    await authVM.checkAuth()
                }
                await container.cacheManager.evictExpired()
            }
            .environment(authViewModel)
            .environment(toastManager)
            .environment(container.connectionMonitor)
        }
    }
}
