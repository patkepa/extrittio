import SwiftUI

struct FadeTransitionModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .transition(.asymmetric(
                insertion: .opacity.combined(with: .offset(y: 6)),
                removal: .opacity
            ))
    }
}

extension View {
    func fadeTransition() -> some View {
        modifier(FadeTransitionModifier())
    }
}
