import Foundation

final class CachingRuleRepository: RuleRepository, Sendable {
    private let remote: any RuleRepository
    private let cacheManager: CacheManager

    init(remote: any RuleRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getRules() async throws -> [Rule] {
        do {
            let rules = try await remote.getRules()
            await cacheManager.cacheRules(rules)
            return rules
        } catch {
            if let cached = await cacheManager.getRules() { return cached }
            throw error
        }
    }

    func toggleRule(id: String, enabled: Bool) async throws -> Rule {
        try await remote.toggleRule(id: id, enabled: enabled)
    }

    func deleteRule(id: String) async throws {
        try await remote.deleteRule(id: id)
        await cacheManager.removeRule(id: id)
    }
}
