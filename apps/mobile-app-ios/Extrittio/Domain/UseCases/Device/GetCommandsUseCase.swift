import Foundation

struct GetCommandsUseCase: Sendable {
    private let repository: any CommandRepository
    init(repository: any CommandRepository) { self.repository = repository }
    func execute(deviceId: String, limit: Int = 50, status: String? = nil) async throws -> [CommandRecord] {
        try await repository.getCommands(deviceId: deviceId, limit: limit, status: status)
    }
}
