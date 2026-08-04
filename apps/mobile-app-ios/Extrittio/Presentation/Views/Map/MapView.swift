import SwiftUI
import MapKit
import CoreLocation

struct MapView: View {
    @State var viewModel: MapViewModel
    @State private var position: MapCameraPosition = .automatic
    @State private var locationManager = CLLocationManager()
    @State private var selectedDevice: Device?
    @State private var selectedDeviceId: String?
    @State private var hasSetInitialRegion = false
    @State private var hasFitDeviceRegion = false
    let container: DependencyContainer

    var body: some View {
        NavigationStack {
            ZStack {
                mapContent
                if viewModel.devicesState.isLoading {
                    LoadingView("Loading map...")
                }
            }
            .safeAreaInset(edge: .top) {
                mapSummaryBar
                    .padding(.horizontal)
                    .padding(.top, Spacing.sm)
            }
            .safeAreaInset(edge: .bottom) {
                if let selectedDevice {
                    selectedDevicePreview(selectedDevice)
                        .padding(.horizontal)
                        .padding(.bottom, Spacing.sm)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
            .animation(AppAnimation.standard.animation, value: selectedDevice)
            .navigationTitle("Map")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Menu {
                        Button {
                            HapticEngine.shared.selection()
                            centerOnOnlineDevices()
                        } label: {
                            Label("Fit Online Devices", systemImage: "antenna.radiowaves.left.and.right")
                        }
                        .disabled(viewModel.onlineDevicesWithLocation.isEmpty)

                        Button {
                            HapticEngine.shared.selection()
                            centerOnDevices()
                        } label: {
                            Label("Fit Devices", systemImage: "location.viewfinder")
                        }
                    } label: {
                        HStack(spacing: 3) {
                            Image(systemName: "location")
                            Image(systemName: "chevron.down")
                                .font(.caption2.weight(.bold))
                        }
                        .accessibilityElement(children: .ignore)
                        .accessibilityLabel("My Location")
                        .accessibilityHint("Tap to center on your location. Touch and hold for map options.")
                    } primaryAction: {
                        HapticEngine.shared.selection()
                        centerOnUserLocation()
                    }
                }
            }
            .task { await viewModel.load() }
            .refreshable { await viewModel.refresh() }
            .navigationDestination(item: $selectedDeviceId) { deviceId in
                DeviceDetailView(deviceId: deviceId, container: container)
            }
        }
    }

    private var mapContent: some View {
        Map(position: $position) {
            UserAnnotation()

            // Device markers
            ForEach(viewModel.devicesWithLocation) { device in
                if let lat = device.latestLatitude, let lon = device.latestLongitude {
                    Annotation(device.name, coordinate: CLLocationCoordinate2D(latitude: lat, longitude: lon)) {
                        deviceMarker(device)
                    }
                }
            }

            // Zone overlays
            ForEach(viewModel.zones) { zone in
                switch zone.geometryJson {
                case .circle(let centerLat, let centerLon, let radiusMeters):
                    MapCircle(
                        center: CLLocationCoordinate2D(latitude: centerLat, longitude: centerLon),
                        radius: radiusMeters
                    )
                    .foregroundStyle(Color(hex: zone.color).opacity(0.15))
                    .stroke(Color(hex: zone.color), lineWidth: 2)

                case .polygon(let points):
                    let coords = points.map { CLLocationCoordinate2D(latitude: $0[0], longitude: $0[1]) }
                    MapPolygon(coordinates: coords)
                        .foregroundStyle(Color(hex: zone.color).opacity(0.15))
                        .stroke(Color(hex: zone.color), lineWidth: 2)
                }
            }
        }
        .mapStyle(.standard(elevation: .flat))
        .onAppear {
            guard !hasSetInitialRegion else { return }
            position = .region(viewModel.mapRegion)
            hasSetInitialRegion = true
        }
        .onChange(of: viewModel.devicesWithLocation.count) { _, _ in
            guard !hasFitDeviceRegion, !viewModel.devicesWithLocation.isEmpty else { return }
            withAnimation {
                position = .region(viewModel.mapRegion)
            }
            hasFitDeviceRegion = true
        }
    }

    private func deviceMarker(_ device: Device) -> some View {
        Button {
            HapticEngine.shared.selection()
            selectedDevice = device
        } label: {
            ZStack {
                Circle()
                    .fill(deviceStatusColor(device.status))
                    .frame(width: 30, height: 30)
                    .shadow(color: deviceStatusColor(device.status).opacity(0.45), radius: 6, y: 2)
                Image(systemName: "sensor.tag.radiowaves.forward")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(.white)
            }
            .overlay(
                Circle()
                    .stroke(.white, lineWidth: 2)
            )
            .overlay {
                if selectedDevice?.id == device.id {
                    Circle()
                        .stroke(deviceStatusColor(device.status), lineWidth: 3)
                        .frame(width: 38, height: 38)
                }
            }
        }
        .buttonStyle(.plain)
    }

    private var mapSummaryBar: some View {
        HStack(spacing: Spacing.md) {
            Label("\(viewModel.devicesWithLocation.count)", systemImage: "location.fill")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.primary)

            Divider()
                .frame(height: 18)

            legendItem("Online", color: .green)
            legendItem("Warning", color: .orange)
            legendItem("Offline", color: .gray)

            Spacer(minLength: 0)

            if !viewModel.zones.isEmpty {
                Label("\(viewModel.zones.count)", systemImage: "map")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, Spacing.md)
        .padding(.vertical, Spacing.sm)
        .glassCard(.elevated)
    }

    private func legendItem(_ label: String, color: Color) -> some View {
        HStack(spacing: Spacing.xs) {
            Circle()
                .fill(color)
                .frame(width: 8, height: 8)
            Text(label)
                .font(.caption2.weight(.medium))
                .foregroundStyle(.secondary)
        }
    }

    private func selectedDevicePreview(_ device: Device) -> some View {
        HStack(alignment: .top, spacing: Spacing.md) {
            Image(systemName: "sensor.tag.radiowaves.forward")
                .font(.title3)
                .foregroundStyle(deviceStatusColor(device.status))
                .frame(width: 38, height: 38)
                .background(deviceStatusColor(device.status).opacity(0.12), in: .rect(cornerRadius: 10))

            VStack(alignment: .leading, spacing: Spacing.xs) {
                HStack(spacing: Spacing.sm) {
                    Text(device.name)
                        .font(.headline)
                        .lineLimit(1)
                    StatusBadge(status: device.status)
                }

                HStack(spacing: Spacing.sm) {
                    if let typeName = device.deviceTypeName {
                        Text(typeName)
                            .lineLimit(1)
                    }
                    if let fleetName = device.fleetName {
                        Text("• \(fleetName)")
                            .lineLimit(1)
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)

                if let lastSeen = lastSeenText(for: device) {
                    Label(lastSeen, systemImage: "clock")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
            }

            Spacer()

            VStack(spacing: Spacing.sm) {
                Button {
                    selectedDevice = nil
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)

                Button {
                    selectedDeviceId = device.id
                } label: {
                    Image(systemName: "chevron.right")
                        .font(.headline)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
            }
        }
        .padding(Spacing.md)
        .glassCard(.elevated)
    }

    private func centerOnUserLocation() {
        if locationManager.authorizationStatus == .notDetermined {
            locationManager.requestWhenInUseAuthorization()
        }

        withAnimation {
            position = .userLocation(followsHeading: false, fallback: .region(viewModel.mapRegion))
        }
    }

    private func centerOnDevices() {
        withAnimation {
            position = .region(viewModel.mapRegion)
        }
    }

    private func centerOnOnlineDevices() {
        withAnimation {
            position = .region(viewModel.onlineDevicesMapRegion)
        }
    }

    private func lastSeenText(for device: Device) -> String? {
        guard let timestamp = device.lastSeen ?? device.lastSeenAt else { return nil }
        return String.formattedTimestamp(timestamp)
    }

    private func deviceStatusColor(_ status: String) -> Color {
        switch status {
        case "online": .green
        case "offline": .gray
        case "warning": .orange
        default: .gray
        }
    }
}
