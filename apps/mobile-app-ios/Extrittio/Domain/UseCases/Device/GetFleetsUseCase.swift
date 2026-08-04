import Foundation

struct GetFleetsUseCase: Sendable {
    private let repository: any FleetRepository
    init(repository: any FleetRepository) { self.repository = repository }
    func execute() async throws -> [Fleet] { try await repository.getFleets() }
}
