import SwiftUI

struct DeviceListSkeletonView: View {
    private let widths: [(CGFloat, CGFloat)] = [
        (150, 100), (170, 90), (130, 110), (160, 95)
    ]

    var body: some View {
        VStack(spacing: Spacing.sm) {
            ForEach(0..<4, id: \.self) { index in
                HStack(spacing: Spacing.md) {
                    SkeletonView(shape: .circle, width: 10, height: 10)
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        SkeletonView(shape: .rect(cornerRadius: 4), width: widths[index].0, height: 14)
                        SkeletonView(shape: .rect(cornerRadius: 4), width: widths[index].1, height: 11)
                    }
                    Spacer()
                    SkeletonView(shape: .capsule, width: 56, height: 22)
                }
                .padding(Spacing.md)
                .glassCard()
            }
        }
        .padding(.horizontal, Spacing.lg)
    }
}
