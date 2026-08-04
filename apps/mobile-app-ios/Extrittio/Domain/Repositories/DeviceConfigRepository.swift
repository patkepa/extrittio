import Foundation

protocol DeviceConfigRepository: Sendable {
    func getConfig(deviceId: String) async throws -> [String: AnyCodableValue]
    func updateConfig(deviceId: String, config: [String: AnyCodableValue]) async throws -> [String: AnyCodableValue]
}
