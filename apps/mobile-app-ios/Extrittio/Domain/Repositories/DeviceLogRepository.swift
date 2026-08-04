import Foundation

protocol DeviceLogRepository: Sendable {
    func getLogs(deviceId: String, limit: Int, level: String?) async throws -> [DeviceLog]
}
