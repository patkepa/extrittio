import SwiftUI

struct StatCard: View {
    let icon: String
    let iconColor: Color
    let label: String
    let value: String
    let descriptor: String
    var highlighted: Bool = false

    @State private var displayValue: Int = 0
    @State private var targetNumber: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: icon)
                    .font(.system(size: 13))
                    .foregroundStyle(iconColor)
                    .frame(width: 28, height: 28)
                    .background(iconColor.opacity(0.15), in: .rect(cornerRadius: 7))

                Text(label)
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(.secondary)
                    .textCase(.uppercase)
                    .tracking(0.5)
            }

            Text(targetNumber != nil ? "\(displayValue)" : value)
                .font(.system(size: 28, weight: .heavy))
                .foregroundStyle(iconColor)
                .contentTransition(.numericText(value: Double(displayValue)))

            Text(descriptor)
                .font(.system(size: 10))
                .foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(
            highlighted ? iconColor.opacity(0.06) : Color.clear,
            in: .rect(cornerRadius: 14)
        )
        .background(.thinMaterial, in: .rect(cornerRadius: 14))
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(highlighted ? iconColor.opacity(0.15) : .clear, lineWidth: 1)
        )
        .onAppear {
            updateDisplayValue(animated: true)
        }
        .onChange(of: value) {
            updateDisplayValue(animated: true)
        }
    }

    private func updateDisplayValue(animated: Bool) {
        guard let target = Int(value) else {
            targetNumber = nil
            return
        }

        targetNumber = target
        let update = { displayValue = target }
        if animated {
            withAnimation(.easeOut(duration: 0.55)) {
                update()
            }
        } else {
            update()
        }
    }
}
