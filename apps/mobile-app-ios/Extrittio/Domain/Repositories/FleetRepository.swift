import Foundation

protocol FleetRepository: Sendable {
    func getFleets() async throws -> [Fleet]
    func createFleet(_ request: CreateFleetRequest) async throws -> Fleet
    func updateFleet(id: Int, _ request: UpdateFleetRequest) async throws -> Fleet
    func deleteFleet(id: Int) async throws
}
