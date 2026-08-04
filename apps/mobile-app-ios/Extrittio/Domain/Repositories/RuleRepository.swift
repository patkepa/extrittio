import Foundation

protocol RuleRepository: Sendable {
    func getRules() async throws -> [Rule]
    func toggleRule(id: String, enabled: Bool) async throws -> Rule
    func deleteRule(id: String) async throws
}
