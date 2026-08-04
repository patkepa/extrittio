import SwiftUI

struct StatusBadge: View {
    let status: String

    @State private var pulseScale: CGFloat = 1.0
    @State private var pulseOpacity: Double = 1.0

    private var color: Color {
        switch status {
        case "online": .green
        case "offline": .red
        case "warning": .yellow
        default: .gray
        }
    }

    var body: some View {
        HStack(spacing: 6) {
            ZStack {
                // Pulse ring (only for online/warning)
                if status == "online" || status == "warning" {
                    Circle()
                        .fill(color)
                        .frame(width: 8, height: 8)
                        .scaleEffect(pulseScale)
                        .opacity(pulseOpacity)
                        .onAppear {
                            let duration = status == "online" ? 2.0 : 1.5
                            withAnimation(.easeInOut(duration: duration).repeatForever(autoreverses: true)) {
                                pulseScale = 1.8
                                pulseOpacity = 0.4
                            }
                        }
                }
                // Main indicator
                Circle()
                    .fill(color)
                    .frame(width: 8, height: 8)
                    .opacity(status == "offline" ? 0.7 : 1)
            }
            Text(status.capitalized)
                .font(.caption.bold())
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .glassEffect()
    }
}
