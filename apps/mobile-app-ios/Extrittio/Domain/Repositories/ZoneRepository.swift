import Foundation

protocol ZoneRepository: Sendable {
    func getZones() async throws -> [Zone]
}
