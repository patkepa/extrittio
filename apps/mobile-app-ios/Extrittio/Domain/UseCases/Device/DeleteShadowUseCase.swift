import Foundation

struct DeleteShadowUseCase: Sendable {
    private let repository: any DeviceShadowRepository
    init(repository: any DeviceShadowRepository) { self.repository = repository }
    func execute(deviceId: String) async throws { try await repository.deleteShadow(deviceId: deviceId) }
}
