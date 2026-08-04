import SwiftUI

struct MetricRow: View {
    let icon: String
    let iconColor: Color
    let title: String
    let subtitle: String?
    let sparklineData: [Double]?
    let progress: Double?
    let value: String
    let valueColor: Color
    let iconSize: CGFloat
    let sparklineWidth: CGFloat

    init(
        icon: String,
        iconColor: Color,
        title: String,
        subtitle: String? = nil,
        sparklineData: [Double]? = nil,
        progress: Double? = nil,
        value: String,
        valueColor: Color? = nil,
        iconSize: CGFloat = 32,
        sparklineWidth: CGFloat = 70
    ) {
        self.icon = icon
        self.iconColor = iconColor
        self.title = title
        self.subtitle = subtitle
        self.sparklineData = sparklineData
        self.progress = progress
        self.value = value
        self.valueColor = valueColor ?? iconColor
        self.iconSize = iconSize
        self.sparklineWidth = sparklineWidth
    }

    var body: some View {
        HStack {
            Image(systemName: icon)
                .font(.system(size: iconSize * 0.44))
                .foregroundStyle(iconColor)
                .frame(width: iconSize, height: iconSize)
                .background(iconColor.opacity(0.15), in: .rect(cornerRadius: iconSize * 0.25))

            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.system(size: 13, weight: .semibold))
                if let subtitle {
                    Text(subtitle)
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            if let data = sparklineData {
                SparklineView(data: data, color: iconColor)
                    .frame(width: sparklineWidth)
            } else if let progress {
                ProgressView(value: min(max(progress, 0), 1))
                    .tint(iconColor)
                    .frame(width: sparklineWidth)
            }

            Text(value)
                .font(.system(size: 17, weight: .bold))
                .foregroundStyle(valueColor)
                .frame(minWidth: 45, alignment: .trailing)
        }
    }
}
