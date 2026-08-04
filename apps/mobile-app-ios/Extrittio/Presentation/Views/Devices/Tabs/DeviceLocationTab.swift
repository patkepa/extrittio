import SwiftUI
import MapKit

struct DeviceLocationTab: View {
    let viewModel: DeviceDetailViewModel
    @State private var position: MapCameraPosition = .automatic

    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                timeRangePicker
                mapSection
                if let location = viewModel.latestLocation {
                    positionInfoSection(location)
                }
            }
            .padding()
        }
        .refreshable { await viewModel.loadLocation(forceRefresh: true) }
        .task { await viewModel.loadLocation() }
    }

    // MARK: - Time Range Picker

    private var timeRangePicker: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(LocationTimeRange.allCases, id: \.self) { range in
                    Button(range.label) {
                        HapticEngine.shared.selection()
                        viewModel.locationTimeRange = range
                        Task { await viewModel.resetLocationAndReload() }
                    }
                    .font(.caption.bold())
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .foregroundStyle(viewModel.locationTimeRange == range ? .white : .primary)
                    .background {
                        if viewModel.locationTimeRange == range {
                            RoundedRectangle(cornerRadius: 8)
                                .fill(.tint)
                        } else {
                            RoundedRectangle(cornerRadius: 8)
                                .fill(.thinMaterial)
                        }
                    }
                    .pressEffect()
                }
            }
        }
    }

    // MARK: - Map

    private var mapSection: some View {
        Group {
            if viewModel.isLoadingLocation && viewModel.locationTrail.isEmpty && viewModel.latestLocation == nil {
                RoundedRectangle(cornerRadius: 16)
                    .fill(.thinMaterial)
                    .frame(height: 350)
                    .overlay(LoadingView("Loading location..."))
            } else if viewModel.latestLocation == nil && viewModel.locationTrail.isEmpty {
                RoundedRectangle(cornerRadius: 16)
                    .fill(.thinMaterial)
                    .frame(height: 350)
                    .overlay(
                        EmptyStateView(
                            icon: "location.slash",
                            title: "No Location Data",
                            message: "This device hasn't reported its location."
                        )
                    )
            } else {
                Map(position: $position) {
                    // Trail polyline
                    if viewModel.locationTrail.count >= 2 {
                        let coords = viewModel.locationTrail.compactMap { record -> CLLocationCoordinate2D? in
                            guard let lat = record.latitude, let lon = record.longitude else { return nil }
                            return CLLocationCoordinate2D(latitude: lat, longitude: lon)
                        }
                        MapPolyline(coordinates: coords)
                            .stroke(.blue.opacity(0.7), style: StrokeStyle(lineWidth: 3, dash: [6, 4]))
                    }

                    // Current position marker
                    if let location = viewModel.latestLocation {
                        Annotation("Current", coordinate: CLLocationCoordinate2D(latitude: location.latitude, longitude: location.longitude)) {
                            ZStack {
                                Circle()
                                    .fill(.blue)
                                    .frame(width: 16, height: 16)
                                Circle()
                                    .stroke(.white, lineWidth: 3)
                                    .frame(width: 16, height: 16)
                                Circle()
                                    .fill(.blue.opacity(0.2))
                                    .frame(width: 32, height: 32)
                            }
                        }
                    }
                }
                .mapStyle(.standard(elevation: .flat))
                .frame(height: 350)
                .clipShape(RoundedRectangle(cornerRadius: 16))
                .onAppear { updateMapPosition() }
                .onChange(of: viewModel.locationTrail.count) { _, _ in updateMapPosition() }
            }
        }
        .slideIn()
    }

    // MARK: - Position Info

    private func positionInfoSection(_ location: DeviceLocation) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Current Position")
                .font(.headline)

            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                metricCard(icon: "location", label: "Latitude", value: String(format: "%.4f\u{00B0}", location.latitude))
                metricCard(icon: "location", label: "Longitude", value: String(format: "%.4f\u{00B0}", location.longitude))

                if let speed = location.speed {
                    metricCard(icon: "speedometer", label: "Speed", value: String(format: "%.1f m/s", speed))
                }
                if let altitude = location.altitude {
                    metricCard(icon: "mountain.2", label: "Altitude", value: String(format: "%.1f m", altitude))
                }
                if let heading = location.heading {
                    metricCard(icon: "safari", label: "Heading", value: String(format: "%.1f\u{00B0}", heading))
                }

                metricCard(icon: "clock", label: "Updated", value: String.formattedTimestamp(location.timestamp))
            }
        }
        .slideIn(delay: 0.1)
    }

    private func metricCard(icon: String, label: String, value: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: icon)
                .font(.body)
                .foregroundStyle(.secondary)
                .frame(width: 24)

            VStack(alignment: .leading, spacing: 2) {
                Text(label)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(value)
                    .font(.subheadline.monospaced())
            }
            Spacer()
        }
        .padding(12)
        .glassCard()
    }

    // MARK: - Helpers

    private func updateMapPosition() {
        let trail = viewModel.locationTrail
        if let location = viewModel.latestLocation {
            if trail.count >= 2 {
                let coords = trail.compactMap { record -> CLLocationCoordinate2D? in
                    guard let lat = record.latitude, let lon = record.longitude else { return nil }
                    return CLLocationCoordinate2D(latitude: lat, longitude: lon)
                }
                let lats = coords.map(\.latitude)
                let lons = coords.map(\.longitude)
                let center = CLLocationCoordinate2D(
                    latitude: (lats.min()! + lats.max()!) / 2,
                    longitude: (lons.min()! + lons.max()!) / 2
                )
                let span = MKCoordinateSpan(
                    latitudeDelta: max((lats.max()! - lats.min()!) * 1.4, 0.005),
                    longitudeDelta: max((lons.max()! - lons.min()!) * 1.4, 0.005)
                )
                withAnimation {
                    position = .region(MKCoordinateRegion(center: center, span: span))
                }
            } else {
                withAnimation {
                    position = .region(MKCoordinateRegion(
                        center: CLLocationCoordinate2D(latitude: location.latitude, longitude: location.longitude),
                        span: MKCoordinateSpan(latitudeDelta: 0.01, longitudeDelta: 0.01)
                    ))
                }
            }
        }
    }
}
