import Foundation

struct DeleteDeviceUseCase: Sendable {
    private let repository: any DeviceRepository
    init(repository: any DeviceRepository) { self.repository = repository }
    func execute(id: String) async throws { try await repository.deleteDevice(id: id) }
}
