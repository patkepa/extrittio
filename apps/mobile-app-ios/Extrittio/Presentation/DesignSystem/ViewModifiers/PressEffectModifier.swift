import SwiftUI

struct PressEffectButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.97 : 1.0)
            .opacity(configuration.isPressed ? 0.9 : 1.0)
            .animation(AppAnimation.quick.animation, value: configuration.isPressed)
    }
}

extension View {
    func pressEffect() -> some View {
        buttonStyle(PressEffectButtonStyle())
    }
}
