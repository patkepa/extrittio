import Foundation
import SwiftData

@Model
final class CachedDeviceType {
    @Attribute(.unique) var id: Int
    var name: String
    var icon: String?
    var colorHex: String?
    var cachedAt: Date

    init(id: Int, name: String, icon: String? = nil, colorHex: String? = nil, cachedAt: Date = Date()) {
        self.id = id
        self.name = name
        self.icon = icon
        self.colorHex = colorHex
        self.cachedAt = cachedAt
    }

    func toDomain() -> DeviceType { DeviceType(id: id, name: name, icon: icon, colorHex: colorHex) }
    static func from(_ dt: DeviceType) -> CachedDeviceType {
        CachedDeviceType(id: dt.id, name: dt.name, icon: dt.icon, colorHex: dt.colorHex)
    }
}
