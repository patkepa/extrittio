import SwiftUI

struct DonutChartView: View {
    let online: Int
    let offline: Int
    let diameter: CGFloat

    private var total: Int { online + offline }

    private var onlineFraction: Double {
        total > 0 ? Double(online) / Double(total) : 0
    }

    var body: some View {
        HStack(spacing: diameter * 0.25) {
            ZStack {
                Circle()
                    .stroke(Color.primary.opacity(0.05), lineWidth: diameter * 0.13)

                Circle()
                    .trim(from: 0, to: onlineFraction)
                    .stroke(Color.green, style: StrokeStyle(lineWidth: diameter * 0.13, lineCap: .round))
                    .rotationEffect(.degrees(-90))

                Circle()
                    .trim(from: onlineFraction, to: 1.0)
                    .stroke(Color.red, style: StrokeStyle(lineWidth: diameter * 0.13, lineCap: .round))
                    .rotationEffect(.degrees(-90))

                VStack(spacing: 0) {
                    Text("\(total)")
                        .font(.system(size: diameter * 0.2, weight: .heavy))
                    Text("total")
                        .font(.system(size: diameter * 0.075))
                        .foregroundStyle(.secondary)
                        .textCase(.uppercase)
                }
            }
            .frame(width: diameter, height: diameter)

            VStack(alignment: .leading, spacing: 12) {
                legendRow(color: .green, count: online, label: "Online")
                legendRow(color: .red, count: offline, label: "Offline")
            }
        }
    }

    private func legendRow(color: Color, count: Int, label: String) -> some View {
        HStack(spacing: 10) {
            Circle()
                .fill(color)
                .frame(width: 10, height: 10)
            VStack(alignment: .leading, spacing: 0) {
                Text("\(count)")
                    .font(.system(size: 14, weight: .bold))
                Text(label)
                    .font(.system(size: 10))
                    .foregroundStyle(.secondary)
            }
        }
    }
}
