import SwiftUI

enum ToastType: Equatable {
    case success(String)
    case error(String)
    case info(String)
    case copied
    case warning(String)

    var message: String {
        switch self {
        case .success(let msg), .error(let msg), .info(let msg), .warning(let msg): msg
        case .copied: "Copied to clipboard"
        }
    }

    var icon: String {
        switch self {
        case .success: "checkmark.circle.fill"
        case .error: "xmark.circle.fill"
        case .info: "info.circle.fill"
        case .copied: "doc.on.doc.fill"
        case .warning: "exclamationmark.triangle.fill"
        }
    }

    var tint: Color {
        switch self {
        case .success: .green
        case .error: .red
        case .info: .blue
        case .copied: .blue
        case .warning: .orange
        }
    }

    var duration: Double {
        switch self {
        case .success, .info: 2.0
        case .error, .warning: 3.0
        case .copied: 1.5
        }
    }
}

@Observable
@MainActor
final class ToastManager {
    var currentToast: ToastType?
    private var queue: [ToastType] = []
    private var dismissTask: Task<Void, Never>?

    func show(_ toast: ToastType) {
        if currentToast != nil {
            queue.append(toast)
        } else {
            present(toast)
        }
    }

    private func present(_ toast: ToastType) {
        currentToast = toast

        switch toast {
        case .success: HapticEngine.shared.success()
        case .error: HapticEngine.shared.error()
        case .warning: HapticEngine.shared.warning()
        case .copied: HapticEngine.shared.impact(.light)
        case .info: break
        }

        dismissTask?.cancel()
        dismissTask = Task {
            try? await Task.sleep(for: .seconds(toast.duration))
            guard !Task.isCancelled else { return }
            dismiss()
        }
    }

    func dismiss() {
        withAnimation(AppAnimation.quick.animation) {
            currentToast = nil
        }
        if !queue.isEmpty {
            let next = queue.removeFirst()
            Task {
                try? await Task.sleep(for: .milliseconds(200))
                present(next)
            }
        }
    }
}
