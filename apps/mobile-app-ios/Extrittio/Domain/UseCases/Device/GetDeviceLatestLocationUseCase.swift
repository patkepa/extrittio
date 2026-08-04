import Foundation

struct GetDeviceLatestLocationUseCase: Sendable {
    private let repository: any DeviceRepository
    init(repository: any DeviceRepository) { self.repository = repository }
    func execute(deviceId: String) async throws -> DeviceLocation? {
        try await repository.getLatestLocation(deviceId: deviceId)
    }
}
