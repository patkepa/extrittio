import Foundation

protocol DeviceShadowRepository: Sendable {
    func getShadow(deviceId: String) async throws -> DeviceShadow?
    func updateDesired(deviceId: String, desired: [String: AnyCodableValue]) async throws -> DeviceShadow
    func deleteShadow(deviceId: String) async throws
}
