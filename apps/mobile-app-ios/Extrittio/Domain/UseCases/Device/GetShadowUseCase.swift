import Foundation

struct GetShadowUseCase: Sendable {
    private let repository: any DeviceShadowRepository
    init(repository: any DeviceShadowRepository) { self.repository = repository }
    func execute(deviceId: String) async throws -> DeviceShadow? { try await repository.getShadow(deviceId: deviceId) }
}
