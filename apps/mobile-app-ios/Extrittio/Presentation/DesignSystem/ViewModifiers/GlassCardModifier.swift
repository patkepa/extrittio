import SwiftUI

enum GlassCardStyle {
    case standard
    case elevated
}

struct GlassCardModifier: ViewModifier {
    let style: GlassCardStyle

    func body(content: Content) -> some View {
        switch style {
        case .standard:
            content
                .background(.thinMaterial, in: .rect(cornerRadius: Spacing.md))
        case .elevated:
            content
                .background(.thinMaterial, in: .rect(cornerRadius: Spacing.md))
                .shadow(color: .black.opacity(0.08), radius: 4, y: 2)
        }
    }
}

extension View {
    func glassCard(_ style: GlassCardStyle = .standard) -> some View {
        modifier(GlassCardModifier(style: style))
    }
}
