import Foundation

struct SendCommandUseCase: Sendable {
    private let repository: any CommandRepository
    init(repository: any CommandRepository) { self.repository = repository }
    func execute(deviceId: String, command: String, params: [String: AnyCodableValue]?) async throws -> CommandRecord {
        try await repository.sendCommand(deviceId: deviceId, command: command, params: params)
    }
}
