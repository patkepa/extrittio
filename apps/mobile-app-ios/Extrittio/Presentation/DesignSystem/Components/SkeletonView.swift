import SwiftUI

enum SkeletonShape {
    case rect(cornerRadius: CGFloat = Spacing.md)
    case circle
    case capsule
}

struct SkeletonView: View {
    let shape: SkeletonShape
    var width: CGFloat? = nil
    var height: CGFloat? = nil

    var body: some View {
        Group {
            switch shape {
            case .rect(let cornerRadius):
                RoundedRectangle(cornerRadius: cornerRadius)
                    .fill(Color.primary.opacity(0.06))
            case .circle:
                Circle()
                    .fill(Color.primary.opacity(0.06))
            case .capsule:
                Capsule()
                    .fill(Color.primary.opacity(0.06))
            }
        }
        .shimmer()
        .frame(width: width, height: height)
    }
}
