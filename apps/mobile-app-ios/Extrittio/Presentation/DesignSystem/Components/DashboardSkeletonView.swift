import SwiftUI

struct DashboardSkeletonView: View {
    var body: some View {
        VStack(spacing: Spacing.md) {
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: Spacing.md) {
                ForEach(0..<4, id: \.self) { _ in
                    SkeletonView(shape: .rect(), height: 72)
                }
            }
            SkeletonView(shape: .rect(), height: 140)
        }
        .padding(Spacing.lg)
    }
}
