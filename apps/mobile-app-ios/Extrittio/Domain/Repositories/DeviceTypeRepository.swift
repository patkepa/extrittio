import Foundation

protocol DeviceTypeRepository: Sendable {
    func getDeviceTypes() async throws -> [DeviceType]
    func createDeviceType(_ request: CreateDeviceTypeRequest) async throws -> DeviceType
    func updateDeviceType(id: Int, _ request: UpdateDeviceTypeRequest) async throws -> DeviceType
    func deleteDeviceType(id: Int) async throws
}
