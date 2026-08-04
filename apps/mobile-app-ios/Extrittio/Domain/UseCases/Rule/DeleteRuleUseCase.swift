import Foundation

struct DeleteRuleUseCase: Sendable {
    private let repository: any RuleRepository
    init(repository: any RuleRepository) { self.repository = repository }
    func execute(id: String) async throws { try await repository.deleteRule(id: id) }
}
