import Foundation

struct UpdateDesiredStateUseCase: Sendable {
    private let repository: any DeviceShadowRepository
    init(repository: any DeviceShadowRepository) { self.repository = repository }
    func execute(deviceId: String, desired: [String: AnyCodableValue]) async throws -> DeviceShadow {
        try await repository.updateDesired(deviceId: deviceId, desired: desired)
    }
}
