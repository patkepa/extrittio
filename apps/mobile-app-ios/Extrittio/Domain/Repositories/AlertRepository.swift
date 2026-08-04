import Foundation

protocol AlertRepository: Sendable {
    func getAlerts(deviceId: String?, status: String?, severity: String?, limit: Int, offset: Int) async throws -> PaginatedResponse<Alert>
    func getAlertSummary() async throws -> AlertSummary
    func acknowledgeAlert(id: String) async throws
    func resolveAlert(id: String) async throws
    func reactivateAlert(id: String) async throws
}
