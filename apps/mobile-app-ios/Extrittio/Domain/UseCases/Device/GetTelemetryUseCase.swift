import Foundation

struct GetTelemetryUseCase: Sendable {
    private let repository: any TelemetryRepository
    init(repository: any TelemetryRepository) { self.repository = repository }
    func execute(deviceId: String, limit: Int = 1000, since: String? = nil) async throws -> [TelemetryRecord] {
        try await repository.getTelemetry(deviceId: deviceId, limit: limit, since: since)
    }
}
