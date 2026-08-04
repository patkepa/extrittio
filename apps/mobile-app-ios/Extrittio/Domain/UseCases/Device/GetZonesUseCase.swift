import Foundation

struct GetZonesUseCase: Sendable {
    private let repository: any ZoneRepository
    init(repository: any ZoneRepository) { self.repository = repository }
    func execute() async throws -> [Zone] {
        try await repository.getZones()
    }
}
