import SwiftUI
import WidgetKit

private struct TapIntoDeviceEntry: TimelineEntry {
    let date: Date
}

private struct TapIntoDeviceProvider: TimelineProvider {
    func placeholder(in context: Context) -> TapIntoDeviceEntry {
        TapIntoDeviceEntry(date: .now)
    }

    func getSnapshot(in context: Context, completion: @escaping (TapIntoDeviceEntry) -> Void) {
        completion(TapIntoDeviceEntry(date: .now))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<TapIntoDeviceEntry>) -> Void) {
        completion(Timeline(entries: [TapIntoDeviceEntry(date: .now)], policy: .never))
    }
}

private struct TapIntoDeviceWidgetView: View {
    @Environment(\.widgetFamily) private var family

    var body: some View {
        content
            .widgetURL(AppDeepLink.pairNearbyDevice.url)
            .containerBackground(for: .widget) {
                Color.accentColor.opacity(0.08)
            }
    }

    @ViewBuilder
    private var content: some View {
        switch family {
        case .accessoryCircular:
            Image(systemName: "dot.radiowaves.left.and.right")
                .font(.title2.weight(.semibold))
                .widgetAccentable()
                .accessibilityLabel("Tap into a device via Bluetooth")
        case .accessoryInline:
            Label("Tap into a device", systemImage: "dot.radiowaves.left.and.right")
        case .accessoryRectangular:
            HStack(spacing: 10) {
                Image(systemName: "dot.radiowaves.left.and.right")
                    .font(.title3.weight(.semibold))
                    .widgetAccentable()

                VStack(alignment: .leading, spacing: 1) {
                    Text("Tap into a device")
                        .font(.headline)
                    Text("Connect via Bluetooth")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        default:
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Image(systemName: "dot.radiowaves.left.and.right")
                        .font(.title2.weight(.semibold))
                        .foregroundStyle(Color.accentColor)
                        .widgetAccentable()

                    Spacer(minLength: 8)

                    Text("BLE")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.secondary)
                }

                Spacer(minLength: 0)

                Text("Tap into\na device")
                    .font(.title3.weight(.semibold))
                    .lineSpacing(-1)

                Text("Connect via Bluetooth")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(.top, 4)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .padding(2)
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Tap into a device. Connect via Bluetooth.")
        }
    }
}

struct ExtrittioPairingWidget: Widget {
    let kind = "ExtrittioPairingWidget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: TapIntoDeviceProvider()) { _ in
            TapIntoDeviceWidgetView()
        }
        .configurationDisplayName("Tap into a device")
        .description("Connect directly to a nearby device via Bluetooth.")
        .supportedFamilies([
            .systemSmall,
            .accessoryCircular,
            .accessoryRectangular,
            .accessoryInline
        ])
    }
}

@main
struct ExtrittioWidgetBundle: WidgetBundle {
    var body: some Widget {
        ExtrittioPairingWidget()
    }
}

#Preview(as: .systemSmall) {
    ExtrittioPairingWidget()
} timeline: {
    TapIntoDeviceEntry(date: .now)
}

#Preview(as: .accessoryRectangular) {
    ExtrittioPairingWidget()
} timeline: {
    TapIntoDeviceEntry(date: .now)
}
