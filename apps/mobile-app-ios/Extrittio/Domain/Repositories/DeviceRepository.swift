import Foundation

protocol DeviceRepository: Sendable {
    func getDevices(status: String?, search: String?, fleetId: Int?, limit: Int, offset: Int) async throws -> PaginatedResponse<Device>
    func getDevice(id: String) async throws -> Device
    func createDevice(_ request: CreateDeviceRequest) async throws -> Device
    func updateDevice(id: String, _ request: UpdateDeviceRequest) async throws -> Device
    func deleteDevice(id: String) async throws
    func restartDevice(id: String) async throws
    func getLatestLocation(deviceId: String) async throws -> DeviceLocation?
}
