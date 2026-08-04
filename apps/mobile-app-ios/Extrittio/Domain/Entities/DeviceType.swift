import Foundation

struct DeviceType: Codable, Sendable, Identifiable {
    let id: Int
    let name: String
    let icon: String?
    let colorHex: String?

    init(id: Int, name: String, icon: String? = nil, colorHex: String? = nil) {
        self.id = id
        self.name = name
        self.icon = icon
        self.colorHex = colorHex
    }

    enum CodingKeys: String, CodingKey {
        case id, name, icon
        case colorHex = "color_hex"
    }
}
