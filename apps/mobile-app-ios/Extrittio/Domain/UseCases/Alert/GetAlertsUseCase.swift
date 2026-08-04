import Foundation

struct GetAlertsUseCase: Sendable {
    private let repository: any AlertRepository
    init(repository: any AlertRepository) { self.repository = repository }
    func execute(deviceId: String? = nil, status: String? = nil, severity: String? = nil, limit: Int = 50, offset: Int = 0) async throws -> PaginatedResponse<Alert> {
        try await repository.getAlerts(deviceId: deviceId, status: status, severity: severity, limit: limit, offset: offset)
    }
}
