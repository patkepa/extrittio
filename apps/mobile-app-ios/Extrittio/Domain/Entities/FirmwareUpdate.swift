import Foundation

struct FirmwareUpdate: Codable, Sendable, Identifiable {
    let id: Int
    let deviceTypeId: Int
    let deviceTypeName: String?
    let version: String
    let url: String
    let sha256: String?
    let description: String?
    let createdAt: String
    let hasBlob: Bool?
    let fileSize: Int?
    let filename: String?
    let source: String?

    enum CodingKeys: String, CodingKey {
        case id, version, url, sha256, description, filename, source
        case deviceTypeId = "device_type_id"
        case deviceTypeName = "device_type_name"
        case createdAt = "created_at"
        case hasBlob = "has_blob"
        case fileSize = "file_size"
    }
}

struct OtaDeployment: Codable, Sendable, Identifiable {
    let id: Int
    let deviceId: String
    let firmwareUpdateId: Int
    let firmwareVersion: String?
    let status: String
    let errorMessage: String?
    let initiatedAt: String
    let completedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, status
        case deviceId = "device_id"
        case firmwareUpdateId = "firmware_update_id"
        case firmwareVersion = "firmware_version"
        case errorMessage = "error_message"
        case initiatedAt = "initiated_at"
        case completedAt = "completed_at"
    }
}
