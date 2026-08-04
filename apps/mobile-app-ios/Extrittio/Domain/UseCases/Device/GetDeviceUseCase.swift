import Foundation

struct GetDeviceUseCase: Sendable {
    private let repository: any DeviceRepository
    init(repository: any DeviceRepository) { self.repository = repository }
    func execute(id: String) async throws -> Device { try await repository.getDevice(id: id) }
}
