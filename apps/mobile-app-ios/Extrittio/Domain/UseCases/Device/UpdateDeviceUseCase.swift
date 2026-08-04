import Foundation

struct UpdateDeviceUseCase: Sendable {
    private let repository: any DeviceRepository
    init(repository: any DeviceRepository) { self.repository = repository }
    func execute(id: String, _ request: UpdateDeviceRequest) async throws -> Device { try await repository.updateDevice(id: id, request) }
}
