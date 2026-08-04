import Foundation

final class RuleRepositoryImpl: RuleRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getRules() async throws -> [Rule] {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.rules(server))
    }

    func toggleRule(id: String, enabled: Bool) async throws -> Rule {
        let server = await apiClient.serverAddress
        let request = ToggleRuleRequest(enabled: enabled)
        return try await apiClient.put(Endpoints.ruleEnabled(server, id: id), body: request)
    }

    func deleteRule(id: String) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.rule(server, id: id))
    }
}
