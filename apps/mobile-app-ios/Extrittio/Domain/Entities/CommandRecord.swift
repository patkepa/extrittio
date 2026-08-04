import Foundation

struct CommandRecord: Codable, Sendable, Identifiable {
    let id: String
    let deviceId: String
    let command: String
    let params: [String: AnyCodableValue]?
    let status: String
    let responsePayload: [String: AnyCodableValue]?
    let createdAt: String
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, command, params, status
        case deviceId = "device_id"
        case responsePayload = "response_payload"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }

    var isPending: Bool { status == "pending" }
    var isSuccess: Bool { status == "success" }
    var isFailed: Bool { status == "failed" }
    var isTimedOut: Bool { status == "timed_out" }
}
