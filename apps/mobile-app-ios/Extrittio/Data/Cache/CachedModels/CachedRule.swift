import Foundation
import SwiftData

@Model
final class CachedRule {
    @Attribute(.unique) var id: String
    var jsonData: Data
    var cachedAt: Date

    init(id: String, jsonData: Data, cachedAt: Date = Date()) {
        self.id = id; self.jsonData = jsonData; self.cachedAt = cachedAt
    }

    func toDomain() -> Rule? {
        try? JSONDecoder().decode(Rule.self, from: jsonData)
    }

    static func from(_ rule: Rule) -> CachedRule {
        let data = (try? JSONEncoder().encode(rule)) ?? Data()
        return CachedRule(id: rule.id, jsonData: data)
    }
}
