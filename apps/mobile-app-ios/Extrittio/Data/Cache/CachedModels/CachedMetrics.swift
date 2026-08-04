import Foundation
import SwiftData

@Model
final class CachedMetrics {
    @Attribute(.unique) var key: String
    var jsonData: Data
    var cachedAt: Date

    init(key: String, jsonData: Data, cachedAt: Date = Date()) {
        self.key = key; self.jsonData = jsonData; self.cachedAt = cachedAt
    }

    func toCurrentMetrics() -> CurrentMetricsResponse? {
        guard key == "current" else { return nil }
        return try? JSONDecoder().decode(CurrentMetricsResponse.self, from: jsonData)
    }

    func toMetricsHistory() -> MetricsHistoryResponse? {
        guard key == "history" else { return nil }
        return try? JSONDecoder().decode(MetricsHistoryResponse.self, from: jsonData)
    }

    static func from(current: CurrentMetricsResponse) -> CachedMetrics {
        let data = (try? JSONEncoder().encode(current)) ?? Data()
        return CachedMetrics(key: "current", jsonData: data)
    }

    static func from(history: MetricsHistoryResponse) -> CachedMetrics {
        let data = (try? JSONEncoder().encode(history)) ?? Data()
        return CachedMetrics(key: "history", jsonData: data)
    }
}
