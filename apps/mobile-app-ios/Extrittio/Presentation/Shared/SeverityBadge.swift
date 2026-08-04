import SwiftUI

struct SeverityBadge: View {
    let severity: String

    private var color: Color {
        switch severity {
        case "critical": .red
        case "warning": .orange
        case "info": .blue
        default: .gray
        }
    }

    private var icon: String {
        switch severity {
        case "critical": "exclamationmark.triangle.fill"
        case "warning": "exclamationmark.circle.fill"
        case "info": "info.circle.fill"
        default: "circle.fill"
        }
    }

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: icon)
                .font(.caption2)
            Text(severity.capitalized)
                .font(.caption.bold())
        }
        .foregroundStyle(color)
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .glassEffect()
    }
}
