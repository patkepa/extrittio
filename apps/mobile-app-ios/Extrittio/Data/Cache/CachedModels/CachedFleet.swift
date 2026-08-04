import Foundation
import SwiftData

@Model
final class CachedFleet {
    @Attribute(.unique) var id: Int
    var name: String
    var deviceCount: Int?
    var cachedAt: Date

    init(id: Int, name: String, deviceCount: Int?, cachedAt: Date = Date()) {
        self.id = id; self.name = name; self.deviceCount = deviceCount; self.cachedAt = cachedAt
    }

    func toDomain() -> Fleet { Fleet(id: id, name: name, deviceCount: deviceCount) }
    static func from(_ fleet: Fleet) -> CachedFleet {
        CachedFleet(id: fleet.id, name: fleet.name, deviceCount: fleet.deviceCount)
    }
}
