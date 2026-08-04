import SwiftUI

struct ToastView: View {
    let toast: ToastType
    let onDismiss: () -> Void

    var body: some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: toast.icon)
                .foregroundStyle(toast.tint)
                .font(.body.weight(.semibold))
            Text(toast.message)
                .font(.subheadline.weight(.medium))
                .foregroundStyle(.primary)
            Spacer()
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.md)
        .glassCard(.elevated)
        .padding(.horizontal, Spacing.lg)
        .transition(.move(edge: .top).combined(with: .opacity))
        .onTapGesture { onDismiss() }
    }
}
