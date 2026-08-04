import Foundation

struct GetDeviceConfigUseCase: Sendable {
    private let repository: any DeviceConfigRepository
    init(repository: any DeviceConfigRepository) { self.repository = repository }
    func execute(deviceId: String) async throws -> [String: AnyCodableValue] { try await repository.getConfig(deviceId: deviceId) }
}
