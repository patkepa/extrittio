import Foundation

final class CachingFleetRepository: FleetRepository, Sendable {
    private let remote: any FleetRepository
    private let cacheManager: CacheManager

    init(remote: any FleetRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getFleets() async throws -> [Fleet] {
        do {
            let fleets = try await remote.getFleets()
            await cacheManager.cacheFleets(fleets)
            return fleets
        } catch {
            if let cached = await cacheManager.getFleets() { return cached }
            throw error
        }
    }

    func createFleet(_ request: CreateFleetRequest) async throws -> Fleet {
        let fleet = try await remote.createFleet(request)
        await refreshCache()
        return fleet
    }

    func updateFleet(id: Int, _ request: UpdateFleetRequest) async throws -> Fleet {
        let fleet = try await remote.updateFleet(id: id, request)
        await refreshCache()
        return fleet
    }

    func deleteFleet(id: Int) async throws {
        try await remote.deleteFleet(id: id)
        await refreshCache()
    }

    private func refreshCache() async {
        if let fleets = try? await remote.getFleets() {
            await cacheManager.cacheFleets(fleets)
        }
    }
}
