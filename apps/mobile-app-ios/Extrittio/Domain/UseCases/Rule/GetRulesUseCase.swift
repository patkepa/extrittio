import Foundation

struct GetRulesUseCase: Sendable {
    private let repository: any RuleRepository
    init(repository: any RuleRepository) { self.repository = repository }
    func execute() async throws -> [Rule] { try await repository.getRules() }
}
