import SwiftUI

struct AdaptiveHStack<Content: View>: View {
    @Environment(\.horizontalSizeClass) private var sizeClass
    var spacing: CGFloat = 16
    @ViewBuilder var content: () -> Content

    var body: some View {
        if sizeClass == .regular {
            HStack(alignment: .top, spacing: spacing) { content() }
        } else {
            VStack(spacing: spacing) { content() }
        }
    }
}
