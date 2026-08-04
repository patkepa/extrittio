import SwiftUI

enum DeviceMockupSize {
    case compact
    case medium
    case large

    var scale: CGFloat {
        switch self {
        case .compact: 0.58
        case .medium: 0.82
        case .large: 1.0
        }
    }

    var frame: CGSize {
        CGSize(width: 112 * scale, height: 88 * scale)
    }
}

enum DeviceMockupVariant: Equatable {
    case sensor
    case tracker
    case gateway
    case camera
    case actuator
    case meter

    static func classify(name: String?, icon: String?) -> DeviceMockupVariant {
        let source = [name, icon]
            .compactMap { $0?.lowercased() }
            .joined(separator: " ")

        if source.containsAny(["camera", "vision", "video", "photo"]) {
            return .camera
        }
        if source.containsAny(["gateway", "hub", "router", "network", "wifi", "wi-fi"]) {
            return .gateway
        }
        if source.containsAny(["gps", "tracker", "location", "asset", "vehicle", "tag"]) {
            return .tracker
        }
        if source.containsAny(["relay", "switch", "actuator", "valve", "motor", "lock"]) {
            return .actuator
        }
        if source.containsAny(["meter", "power", "energy", "battery", "pressure", "gauge"]) {
            return .meter
        }
        return .sensor
    }

    var fallbackSymbol: String {
        switch self {
        case .sensor: "sensor.tag.radiowaves.forward"
        case .tracker: "location.fill"
        case .gateway: "network"
        case .camera: "camera.fill"
        case .actuator: "switch.2"
        case .meter: "gauge.with.dots.needle.33percent"
        }
    }

    var fallbackColor: Color {
        switch self {
        case .sensor: .blue
        case .tracker: .mint
        case .gateway: .purple
        case .camera: .indigo
        case .actuator: .orange
        case .meter: .teal
        }
    }
}

struct DeviceMockupView: View {
    let deviceTypeName: String?
    let icon: String?
    let colorHex: String?
    let status: String?
    let size: DeviceMockupSize

    init(
        deviceTypeName: String? = nil,
        icon: String? = nil,
        colorHex: String? = nil,
        status: String? = nil,
        size: DeviceMockupSize = .medium
    ) {
        self.deviceTypeName = deviceTypeName
        self.icon = icon
        self.colorHex = colorHex
        self.status = status
        self.size = size
    }

    private var variant: DeviceMockupVariant {
        DeviceMockupVariant.classify(name: deviceTypeName, icon: icon)
    }

    private var baseColor: Color {
        if let colorHex, !colorHex.isEmpty {
            return Color(hex: colorHex)
        }
        return variant.fallbackColor
    }

    private var displaySymbol: String {
        guard let icon, !icon.isEmpty else { return variant.fallbackSymbol }
        return icon
    }

    private var statusColor: Color {
        switch status {
        case "online": .green
        case "offline": .gray
        case "warning": .orange
        default: baseColor
        }
    }

    var body: some View {
        ZStack {
            floorShadow
            mockupBody
            if status != nil {
                statusDot
            }
        }
        .frame(width: size.frame.width, height: size.frame.height)
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private var mockupBody: some View {
        switch variant {
        case .sensor:
            sensorBody
        case .tracker:
            trackerBody
        case .gateway:
            gatewayBody
        case .camera:
            cameraBody
        case .actuator:
            actuatorBody
        case .meter:
            meterBody
        }
    }

    private var floorShadow: some View {
        Ellipse()
            .fill(.black.opacity(0.14))
            .frame(width: value(76), height: value(18))
            .blur(radius: value(5))
            .offset(x: value(6), y: value(30))
    }

    private var statusDot: some View {
        Circle()
            .fill(statusColor)
            .frame(width: value(11), height: value(11))
            .overlay(
                Circle()
                    .stroke(.white.opacity(0.9), lineWidth: value(2))
            )
            .shadow(color: statusColor.opacity(0.45), radius: value(4))
            .offset(x: value(37), y: value(-27))
    }

    private var sensorBody: some View {
        isometric {
            ZStack {
                enclosure(width: 62, height: 44, cornerRadius: 12)
                Image(systemName: displaySymbol)
                    .font(.system(size: value(18), weight: .semibold))
                    .foregroundStyle(.white)
                    .shadow(color: .black.opacity(0.18), radius: value(2), y: value(1))
                HStack(spacing: value(6)) {
                    ForEach(0..<3, id: \.self) { index in
                        Circle()
                            .fill(.white.opacity(index == 0 ? 0.9 : 0.48))
                            .frame(width: value(5), height: value(5))
                    }
                }
                .offset(y: value(15))
            }
        }
    }

    private var trackerBody: some View {
        isometric {
            ZStack {
                enclosure(width: 48, height: 62, cornerRadius: 22)
                RoundedRectangle(cornerRadius: value(18), style: .continuous)
                    .stroke(.white.opacity(0.28), lineWidth: value(2))
                    .frame(width: value(34), height: value(48))
                Image(systemName: displaySymbol)
                    .font(.system(size: value(20), weight: .bold))
                    .foregroundStyle(.white)
                    .offset(y: value(-2))
                Circle()
                    .fill(.white.opacity(0.7))
                    .frame(width: value(6), height: value(6))
                    .offset(y: value(21))
            }
        }
    }

    private var gatewayBody: some View {
        isometric {
            ZStack {
                antenna(xOffset: -22, height: 30)
                antenna(xOffset: 22, height: 24)
                enclosure(width: 70, height: 46, cornerRadius: 10)
                Image(systemName: displaySymbol)
                    .font(.system(size: value(21), weight: .semibold))
                    .foregroundStyle(.white)
                    .offset(y: value(-2))
                HStack(spacing: value(6)) {
                    ForEach(0..<4, id: \.self) { index in
                        RoundedRectangle(cornerRadius: value(2), style: .continuous)
                            .fill(.white.opacity(index == 0 ? 0.9 : 0.42))
                            .frame(width: value(8), height: value(3))
                    }
                }
                .offset(y: value(17))
            }
        }
    }

    private var cameraBody: some View {
        isometric {
            ZStack {
                enclosure(width: 72, height: 44, cornerRadius: 12)
                Circle()
                    .fill(.black.opacity(0.2))
                    .frame(width: value(31), height: value(31))
                    .overlay(
                        Circle()
                            .stroke(.white.opacity(0.72), lineWidth: value(3))
                    )
                Circle()
                    .fill(
                        LinearGradient(
                            colors: [.white.opacity(0.92), baseColor.opacity(0.35)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                    .frame(width: value(16), height: value(16))
                RoundedRectangle(cornerRadius: value(3), style: .continuous)
                    .fill(.white.opacity(0.75))
                    .frame(width: value(9), height: value(6))
                    .offset(x: value(24), y: value(-13))
            }
        }
    }

    private var actuatorBody: some View {
        isometric {
            ZStack {
                enclosure(width: 66, height: 46, cornerRadius: 10)
                Capsule()
                    .fill(.black.opacity(0.18))
                    .frame(width: value(37), height: value(17))
                    .overlay(alignment: .trailing) {
                        Circle()
                            .fill(.white.opacity(0.92))
                            .frame(width: value(13), height: value(13))
                            .padding(.trailing, value(2))
                    }
                    .offset(y: value(-7))
                Image(systemName: displaySymbol)
                    .font(.system(size: value(16), weight: .semibold))
                    .foregroundStyle(.white.opacity(0.9))
                    .offset(y: value(14))
            }
        }
    }

    private var meterBody: some View {
        isometric {
            ZStack {
                enclosure(width: 58, height: 54, cornerRadius: 14)
                Circle()
                    .trim(from: 0.58, to: 0.94)
                    .stroke(.white.opacity(0.78), style: StrokeStyle(lineWidth: value(4), lineCap: .round))
                    .frame(width: value(34), height: value(34))
                    .rotationEffect(.degrees(180))
                    .offset(y: value(2))
                Rectangle()
                    .fill(.white.opacity(0.86))
                    .frame(width: value(3), height: value(17))
                    .rotationEffect(.degrees(42))
                    .offset(x: value(5), y: value(7))
                Circle()
                    .fill(.white.opacity(0.94))
                    .frame(width: value(6), height: value(6))
                    .offset(y: value(10))
            }
        }
    }

    private func enclosure(width: CGFloat, height: CGFloat, cornerRadius: CGFloat) -> some View {
        ZStack {
            RoundedRectangle(cornerRadius: value(cornerRadius), style: .continuous)
                .fill(baseColor.opacity(0.42))
                .frame(width: value(width), height: value(height))
                .offset(x: value(8), y: value(8))
            RoundedRectangle(cornerRadius: value(cornerRadius), style: .continuous)
                .fill(
                    LinearGradient(
                        colors: [
                            baseColor.opacity(0.98),
                            baseColor.opacity(0.78),
                            baseColor.opacity(0.52)
                        ],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .frame(width: value(width), height: value(height))
                .overlay(
                    RoundedRectangle(cornerRadius: value(cornerRadius), style: .continuous)
                        .stroke(.white.opacity(0.26), lineWidth: value(1))
                )
                .overlay(alignment: .topLeading) {
                    RoundedRectangle(cornerRadius: value(cornerRadius - 4), style: .continuous)
                        .fill(.white.opacity(0.18))
                        .frame(width: value(width * 0.5), height: value(height * 0.28))
                        .padding(value(5))
                }
        }
        .shadow(color: baseColor.opacity(0.22), radius: value(8), x: value(2), y: value(7))
    }

    private func antenna(xOffset: CGFloat, height: CGFloat) -> some View {
        Capsule()
            .fill(
                LinearGradient(
                    colors: [baseColor.opacity(0.85), baseColor.opacity(0.35)],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )
            .frame(width: value(4), height: value(height))
            .offset(x: value(xOffset), y: value(-24))
    }

    private func isometric<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        content()
            .rotation3DEffect(.degrees(20), axis: (x: 1, y: -0.7, z: 0))
            .rotationEffect(.degrees(-6))
    }

    private func value(_ base: CGFloat) -> CGFloat {
        base * size.scale
    }
}

private extension String {
    func containsAny(_ candidates: [String]) -> Bool {
        candidates.contains { contains($0) }
    }
}
