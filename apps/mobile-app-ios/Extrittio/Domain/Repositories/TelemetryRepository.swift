import Foundation

protocol TelemetryRepository: Sendable {
    func getTelemetry(deviceId: String, limit: Int, since: String?) async throws -> [TelemetryRecord]
}
