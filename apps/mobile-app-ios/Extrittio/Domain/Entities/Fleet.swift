import Foundation

struct Fleet: Codable, Sendable, Identifiable {
    let id: Int
    let name: String
    let deviceCount: Int?

    enum CodingKeys: String, CodingKey {
        case id, name
        case deviceCount = "device_count"
    }
}
