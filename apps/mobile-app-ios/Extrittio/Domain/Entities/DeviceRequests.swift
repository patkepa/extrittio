import Foundation

struct CreateDeviceRequest: Codable, Sendable {
    let name: String
    let deviceTypeId: Int?
    let fleetId: Int?
    let firmware: String?
    let blueprintRevisionId: String?
    let configuration: AnyCodableValue?

    init(
        name: String,
        deviceTypeId: Int? = nil,
        fleetId: Int? = nil,
        firmware: String? = nil,
        blueprintRevisionId: String? = nil,
        configuration: AnyCodableValue? = nil
    ) {
        self.name = name
        self.deviceTypeId = deviceTypeId
        self.fleetId = fleetId
        self.firmware = firmware
        self.blueprintRevisionId = blueprintRevisionId
        self.configuration = configuration
    }

    enum CodingKeys: String, CodingKey {
        case name, firmware, configuration
        case deviceTypeId = "device_type_id"
        case fleetId = "fleet_id"
        case blueprintRevisionId = "blueprint_revision_id"
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
