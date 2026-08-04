import Foundation

struct ResolveAlertUseCase: Sendable {
    private let repository: any AlertRepository
    init(repository: any AlertRepository) { self.repository = repository }
    func execute(id: String) async throws { try await repository.resolveAlert(id: id) }
}
