import SwiftUI

struct OfflineBanner: View {
    @Environment(ConnectionMonitor.self) private var connectionMonitor

    var body: some View {
        if !connectionMonitor.isOnline {
            Label("You're offline — showing cached data", systemImage: "wifi.slash")
                .font(.caption)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity)
                .background(.ultraThinMaterial)
        }
    }
}

struct OfflineActionHint: View {
    var message = "Actions are unavailable while offline."

    var body: some View {
        Label(message, systemImage: "wifi.slash")
            .font(.caption)
            .foregroundStyle(.secondary)
            .padding(.horizontal, Spacing.md)
            .padding(.vertical, Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .glassCard()
    }
}

struct StalenessLabel: View {
    let date: Date

    var body: some View {
        Text("Last updated \(date, format: .relative(presentation: .named))")
            .font(.caption2)
            .foregroundStyle(.secondary)
    }
}
