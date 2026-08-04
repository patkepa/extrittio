import Foundation

struct ToggleRuleUseCase: Sendable {
    private let repository: any RuleRepository
    init(repository: any RuleRepository) { self.repository = repository }
    func execute(id: String, enabled: Bool) async throws -> Rule { try await repository.toggleRule(id: id, enabled: enabled) }
}
