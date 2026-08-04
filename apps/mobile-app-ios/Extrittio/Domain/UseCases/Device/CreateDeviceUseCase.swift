import Foundation

struct CreateDeviceUseCase: Sendable {
    private let repository: any DeviceRepository
    init(repository: any DeviceRepository) { self.repository = repository }
    func execute(_ request: CreateDeviceRequest) async throws -> Device { try await repository.createDevice(request) }
}
