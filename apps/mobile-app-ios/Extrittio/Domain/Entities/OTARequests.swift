import Foundation

struct TriggerOTARequest: Codable, Sendable {
    let firmwareUpdateId: Int

    enum CodingKeys: String, CodingKey {
        case firmwareUpdateId = "firmware_update_id"
    }
}
