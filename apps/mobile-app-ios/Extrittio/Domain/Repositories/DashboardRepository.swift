import Foundation

protocol DashboardRepository: Sendable {
    func getStats() async throws -> DashboardStats
}
