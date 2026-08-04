import Foundation

struct UpdateDeviceConfigUseCase: Sendable {
    private let repository: any DeviceConfigRepository
    init(repository: any DeviceConfigRepository) { self.repository = repository }
    func execute(deviceId: String, config: [String: AnyCodableValue]) async throws -> [String: AnyCodableValue] {
        try await repository.updateConfig(deviceId: deviceId, config: config)
    }
}
