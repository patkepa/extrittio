import SwiftUI

struct AdminRowShell<Content: View>: View {
    var isWorking = false
    private let content: Content

    init(isWorking: Bool = false, @ViewBuilder content: () -> Content) {
        self.isWorking = isWorking
        self.content = content()
    }

    var body: some View {
        content
            .padding(Spacing.md)
            .glassCard()
            .opacity(isWorking ? 0.65 : 1)
    }
}

struct AdminMetadataBadge: View {
    let text: String
    var tint: Color = .secondary

    var body: some View {
        Text(text)
            .font(.caption2.weight(.semibold))
            .foregroundStyle(tint)
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, 3)
            .background(tint.opacity(0.12), in: .capsule)
    }
}

struct AdminSkeletonRows: View {
    var count = 4

    var body: some View {
        ForEach(0..<count, id: \.self) { _ in
            AdminRowShell {
                HStack(spacing: Spacing.md) {
                    SkeletonView(shape: .circle, width: 32, height: 32)
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        SkeletonView(shape: .rect(cornerRadius: 4), width: 150, height: 14)
                        SkeletonView(shape: .rect(cornerRadius: 4), width: 96, height: 11)
                    }
                    Spacer()
                    SkeletonView(shape: .circle, width: 24, height: 24)
                }
            }
            .adminListRow()
        }
    }
}

extension View {
    func adminListRow() -> some View {
        self
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: Spacing.xs, leading: Spacing.lg, bottom: Spacing.xs, trailing: Spacing.lg))
    }
}
