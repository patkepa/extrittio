import SwiftUI

struct ChartSkeletonView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            SkeletonView(shape: .rect(cornerRadius: 4), width: 120, height: 14)
            ZStack {
                RoundedRectangle(cornerRadius: Spacing.md)
                    .fill(Color.primary.opacity(0.02))
                VStack(spacing: 0) {
                    ForEach(0..<3, id: \.self) { _ in
                        Spacer()
                        Rectangle()
                            .fill(Color.primary.opacity(0.04))
                            .frame(height: 1)
                    }
                    Spacer()
                }
                .padding(.horizontal, Spacing.lg)
            }
            .frame(height: 120)
            .shimmer()
        }
    }
}
