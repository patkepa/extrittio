import Foundation
import os

@Observable
@MainActor
final class RulesViewModel {
    var rulesState: ViewState<[Rule]> = .loading
    var errorMessage: String?

    private let getRulesUseCase: GetRulesUseCase
    private let toggleRuleUseCase: ToggleRuleUseCase
    private let deleteRuleUseCase: DeleteRuleUseCase
    private let cacheMetadata: any CacheMetadataProvider
    private let connectionMonitor: ConnectionMonitor
    private let logger = Logger(subsystem: "com.extrittio", category: "Rules")

    init(getRulesUseCase: GetRulesUseCase, toggleRuleUseCase: ToggleRuleUseCase, deleteRuleUseCase: DeleteRuleUseCase, cacheMetadata: any CacheMetadataProvider, connectionMonitor: ConnectionMonitor) {
        self.getRulesUseCase = getRulesUseCase
        self.toggleRuleUseCase = toggleRuleUseCase
        self.deleteRuleUseCase = deleteRuleUseCase
        self.cacheMetadata = cacheMetadata
        self.connectionMonitor = connectionMonitor
    }

    func load() async {
        rulesState = .loading
        errorMessage = nil
        do {
            let rules = try await getRulesUseCase.execute()
            if rules.isEmpty {
                rulesState = .empty
            } else if !connectionMonitor.isOnline, let lastUpdated = await cacheMetadata.lastUpdated(for: .rules) {
                rulesState = .cached(rules, lastUpdated: lastUpdated)
            } else {
                rulesState = .loaded(rules)
            }
        } catch {
            rulesState = .error(error)
            logger.error("Rules load failed: \(error)")
        }
    }

    @discardableResult
    func toggleRule(id: String, enabled: Bool) async -> Bool {
        do {
            let updated = try await toggleRuleUseCase.execute(id: id, enabled: enabled)
            if var rules = rulesState.data,
               let index = rules.firstIndex(where: { $0.id == id }) {
                rules[index] = updated
                rulesState = .loaded(rules)
            }
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    @discardableResult
    func deleteRule(id: String) async -> Bool {
        do {
            try await deleteRuleUseCase.execute(id: id)
            if var rules = rulesState.data {
                rules.removeAll { $0.id == id }
                rulesState = rules.isEmpty ? .empty : .loaded(rules)
            }
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }
}
