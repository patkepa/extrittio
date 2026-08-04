import Foundation
import SwiftData

@Model
final class CachedCommandRecord {
    @Attribute(.unique) var id: String
    var deviceId: String
    var command: String
    var paramsData: Data?
    var status: String
    var responsePayloadData: Data?
    var createdAt: String
    var updatedAt: String
    var cachedAt: Date

    init(id: String, deviceId: String, command: String, paramsData: Data?,
         status: String, responsePayloadData: Data?, createdAt: String,
         updatedAt: String, cachedAt: Date = Date()) {
        self.id = id; self.deviceId = deviceId; self.command = command
        self.paramsData = paramsData; self.status = status
        self.responsePayloadData = responsePayloadData
        self.createdAt = createdAt; self.updatedAt = updatedAt; self.cachedAt = cachedAt
    }

    func toDomain() -> CommandRecord {
        let decoder = JSONDecoder()
        let params: [String: AnyCodableValue]? = paramsData.flatMap { try? decoder.decode([String: AnyCodableValue].self, from: $0) }
        let response: [String: AnyCodableValue]? = responsePayloadData.flatMap { try? decoder.decode([String: AnyCodableValue].self, from: $0) }
        return CommandRecord(id: id, deviceId: deviceId, command: command, params: params,
                             status: status, responsePayload: response,
                             createdAt: createdAt, updatedAt: updatedAt)
    }

    static func from(_ cmd: CommandRecord) -> CachedCommandRecord {
        let encoder = JSONEncoder()
        let params = cmd.params.flatMap { try? encoder.encode($0) }
        let response = cmd.responsePayload.flatMap { try? encoder.encode($0) }
        return CachedCommandRecord(id: cmd.id, deviceId: cmd.deviceId, command: cmd.command,
                                   paramsData: params, status: cmd.status,
                                   responsePayloadData: response,
                                   createdAt: cmd.createdAt, updatedAt: cmd.updatedAt)
    }
}
