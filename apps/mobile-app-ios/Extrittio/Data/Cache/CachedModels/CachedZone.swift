import Foundation
import SwiftData

@Model
final class CachedZone {
    @Attribute(.unique) var id: String
    var jsonData: Data
    var cachedAt: Date

    init(id: String, jsonData: Data, cachedAt: Date = Date()) {
        self.id = id; self.jsonData = jsonData; self.cachedAt = cachedAt
    }

    func toDomain() -> Zone? {
        try? JSONDecoder().decode(Zone.self, from: jsonData)
    }

    static func from(_ zone: Zone) -> CachedZone {
        let data = (try? JSONEncoder().encode(zone)) ?? Data()
        return CachedZone(id: zone.id, jsonData: data)
    }
}
