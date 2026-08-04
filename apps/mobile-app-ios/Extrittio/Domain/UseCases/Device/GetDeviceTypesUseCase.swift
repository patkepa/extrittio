import Foundation

struct GetDeviceTypesUseCase: Sendable {
    private let repository: any DeviceTypeRepository
    init(repository: any DeviceTypeRepository) { self.repository = repository }
    func execute() async throws -> [DeviceType] { try await repository.getDeviceTypes() }
}
