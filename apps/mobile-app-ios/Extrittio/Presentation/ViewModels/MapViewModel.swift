import Foundation
import os
import MapKit

@Observable
@MainActor
final class MapViewModel {
    var devicesState: ViewState<[Device]> = .loading
    var zonesState: ViewState<[Zone]> = .loading

    var devicesWithLocation: [Device] {
        (devicesState.data ?? []).filter { $0.hasLocation }
    }

    var onlineDevicesWithLocation: [Device] {
        devicesWithLocation.filter(\.isOnline)
    }

    var zones: [Zone] { zonesState.data ?? [] }

    private let getDevicesUseCase: GetDevicesUseCase
    private let getZonesUseCase: GetZonesUseCase
    private let cacheMetadata: any CacheMetadataProvider
    private let connectionMonitor: ConnectionMonitor
    private let logger = Logger(subsystem: "com.extrittio", category: "Map")

    init(getDevicesUseCase: GetDevicesUseCase, getZonesUseCase: GetZonesUseCase, cacheMetadata: any CacheMetadataProvider, connectionMonitor: ConnectionMonitor) {
        self.getDevicesUseCase = getDevicesUseCase
        self.getZonesUseCase = getZonesUseCase
        self.cacheMetadata = cacheMetadata
        self.connectionMonitor = connectionMonitor
    }

    func load() async {
        devicesState = .loading
        zonesState = .loading
        let isOffline = !connectionMonitor.isOnline

        async let devicesTask: Void = loadDevices(isOffline: isOffline)
        async let zonesTask: Void = loadZones(isOffline: isOffline)
        _ = await (devicesTask, zonesTask)
    }

    private func loadDevices(isOffline: Bool) async {
        do {
            let devices = try await getDevicesUseCase.execute(
                status: nil, search: nil, fleetId: nil, limit: 1000, offset: 0
            ).data
            if devices.isEmpty {
                devicesState = .empty
            } else if isOffline, let lastUpdated = await cacheMetadata.lastUpdated(for: .devices) {
                devicesState = .cached(devices, lastUpdated: lastUpdated)
            } else {
                devicesState = .loaded(devices)
            }
        } catch {
            devicesState = .error(error)
            logger.error("Map devices load failed: \(error)")
        }
    }

    private func loadZones(isOffline: Bool) async {
        do {
            let zones = try await getZonesUseCase.execute()
            if zones.isEmpty {
                zonesState = .empty
            } else if isOffline, let lastUpdated = await cacheMetadata.lastUpdated(for: .zones) {
                zonesState = .cached(zones, lastUpdated: lastUpdated)
            } else {
                zonesState = .loaded(zones)
            }
        } catch {
            zonesState = .error(error)
            logger.error("Map zones load failed: \(error)")
        }
    }

    func refresh() async {
        await load()
    }

    var mapRegion: MKCoordinateRegion {
        region(for: devicesWithLocation)
    }

    var onlineDevicesMapRegion: MKCoordinateRegion {
        region(for: onlineDevicesWithLocation)
    }

    private func region(for devices: [Device]) -> MKCoordinateRegion {
        let lats = devices.compactMap(\.latestLatitude)
        let lons = devices.compactMap(\.latestLongitude)
        guard !lats.isEmpty, !lons.isEmpty else {
            return defaultRegion
        }

        let minLat = lats.min()!
        let maxLat = lats.max()!
        let minLon = lons.min()!
        let maxLon = lons.max()!
        let center = CLLocationCoordinate2D(
            latitude: (minLat + maxLat) / 2,
            longitude: (minLon + maxLon) / 2
        )
        let span = MKCoordinateSpan(
            latitudeDelta: max((maxLat - minLat) * 1.4, 0.01),
            longitudeDelta: max((maxLon - minLon) * 1.4, 0.01)
        )
        return MKCoordinateRegion(center: center, span: span)
    }

    private var defaultRegion: MKCoordinateRegion {
        MKCoordinateRegion(
            center: CLLocationCoordinate2D(latitude: 52.2297, longitude: 21.0122),
            span: MKCoordinateSpan(latitudeDelta: 0.1, longitudeDelta: 0.1)
        )
    }
}
