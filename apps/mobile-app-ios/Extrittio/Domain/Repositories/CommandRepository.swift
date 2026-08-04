import Foundation

protocol CommandRepository: Sendable {
    func getCommands(deviceId: String, limit: Int, status: String?) async throws -> [CommandRecord]
    func sendCommand(deviceId: String, command: String, params: [String: AnyCodableValue]?) async throws -> CommandRecord
}
