import Charts
import SwiftUI

struct SparklineView: View {
    let data: [Double]
    let color: Color
    let height: CGFloat
    let lineWidth: CGFloat

    init(data: [Double], color: Color, height: CGFloat = 24, lineWidth: CGFloat = 1.5) {
        self.data = data
        self.color = color
        self.height = height
        self.lineWidth = lineWidth
    }

    var body: some View {
        if data.count >= 2 {
            Chart {
                ForEach(Array(data.enumerated()), id: \.offset) { index, value in
                    LineMark(
                        x: .value("Time", index),
                        y: .value("Value", value)
                    )
                    .interpolationMethod(.catmullRom)
                    .foregroundStyle(color)
                    .lineStyle(StrokeStyle(lineWidth: lineWidth))

                    AreaMark(
                        x: .value("Time", index),
                        y: .value("Value", value)
                    )
                    .interpolationMethod(.catmullRom)
                    .foregroundStyle(
                        LinearGradient(
                            colors: [color.opacity(0.3), color.opacity(0.0)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                    )
                }
            }
            .chartXAxis(.hidden)
            .chartYAxis(.hidden)
            .chartLegend(.hidden)
            .chartYScale(domain: .automatic(includesZero: false))
            .chartBackground { _ in Color.clear }
            .frame(height: height)
        } else {
            RoundedRectangle(cornerRadius: 3)
                .fill(color.opacity(0.1))
                .frame(height: height)
                .overlay {
                    if data.isEmpty {
                        Text("—")
                            .font(.system(size: 9))
                            .foregroundStyle(color.opacity(0.3))
                    }
                }
        }
    }
}
