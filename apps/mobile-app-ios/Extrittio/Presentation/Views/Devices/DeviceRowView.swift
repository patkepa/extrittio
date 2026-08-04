import SwiftUI

struct DeviceRowView: View {
    let device: Device

    private var lastSeenText: String? {
        guard let timestamp = device.lastSeen ?? device.lastSeenAt else { return nil }
        return String.formattedTimestamp(timestamp)
    }

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.md) {
            DeviceMockupView(
                deviceTypeName: device.deviceTypeName,
                status: device.status,
                size: .compact
            )
            .frame(width: 54, height: 48)

            VStack(alignment: .leading, spacing: Spacing.xs + 1) {
                HStack(spacing: Spacing.xs) {
                    Text(device.name)
                        .font(.body.bold())
                        .lineLimit(1)
                    if device.hasLocation {
                        Image(systemName: "location.fill")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }

                HStack(spacing: Spacing.sm) {
                    if let typeName = device.deviceTypeName {
                        Text(typeName)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    if let fleetName = device.fleetName {
                        Text("• \(fleetName)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                }

                HStack(spacing: Spacing.md) {
                    if let lastSeenText {
                        Label(lastSeenText, systemImage: "clock")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                            .lineLimit(1)
                    }
                    if device.uptimeSeconds > 0 {
                        Label(formatUptime(device.uptimeSeconds), systemImage: "timer")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                            .lineLimit(1)
                    }
                }
            }

            Spacer()

            VStack(alignment: .trailing, spacing: Spacing.xs) {
                StatusBadge(status: device.status)
                Text(device.firmwareVersionLabel)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(Spacing.md)
        .glassCard()
    }

    private func formatUptime(_ seconds: Int) -> String {
        let days = seconds / 86400
        let hours = (seconds % 86400) / 3600
        let mins = (seconds % 3600) / 60
        if days > 0 { return "\(days)d \(hours)h" }
        if hours > 0 { return "\(hours)h \(mins)m" }
        return "\(mins)m"
    }
}
