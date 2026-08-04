import Foundation

struct CreateDeviceRequest: Codable, Sendable {
    let name: String
    let deviceTypeId: Int
    let fleetId: Int?
    let firmware: String?

    enum CodingKeys: String, CodingKey {
        case name, firmware
        case deviceTypeId = "device_type_id"
        case fleetId = "fleet_id"
    }
}

struct UpdateDeviceRequest: Codable, Sendable {
    let name: String?
    let deviceTypeId: Int?
    let fleetId: Int?
    let firmware: String?

    enum CodingKeys: String, CodingKey {
        case name, firmware
        case deviceTypeId = "device_type_id"
        case fleetId = "fleet_id"
    }
}
