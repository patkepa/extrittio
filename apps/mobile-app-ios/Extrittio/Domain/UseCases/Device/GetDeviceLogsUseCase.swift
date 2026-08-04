import Foundation

struct GetDeviceLogsUseCase: Sendable {
    private let repository: any DeviceLogRepository
    init(repository: any DeviceLogRepository) { self.repository = repository }
    func execute(deviceId: String, limit: Int = 200, level: String? = nil) async throws -> [DeviceLog] {
        try await repository.getLogs(deviceId: deviceId, limit: limit, level: level)
    }
}
