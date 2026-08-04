import Foundation

struct GetDevicesUseCase: Sendable {
    private let repository: any DeviceRepository
    init(repository: any DeviceRepository) { self.repository = repository }

    func execute(status: String? = nil, search: String? = nil, fleetId: Int? = nil, limit: Int = 30, offset: Int = 0) async throws -> PaginatedResponse<Device> {
        try await repository.getDevices(status: status, search: search, fleetId: fleetId, limit: limit, offset: offset)
    }
}
