import Foundation

final class CachingCommandRepository: CommandRepository, Sendable {
    private let remote: any CommandRepository
    private let cacheManager: CacheManager

    init(remote: any CommandRepository, cacheManager: CacheManager) {
        self.remote = remote
        self.cacheManager = cacheManager
    }

    func getCommands(deviceId: String, limit: Int, status: String?) async throws -> [CommandRecord] {
        do {
            let commands = try await remote.getCommands(deviceId: deviceId, limit: limit, status: status)
            await cacheManager.cacheCommands(commands, deviceId: deviceId)
            return commands
        } catch {
            if let cached = await cacheManager.getCommands(deviceId: deviceId) { return cached }
            throw error
        }
    }

    func sendCommand(deviceId: String, command: String, params: [String: AnyCodableValue]?) async throws -> CommandRecord {
        try await remote.sendCommand(deviceId: deviceId, command: command, params: params)
    }
}
