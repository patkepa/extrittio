import Foundation

struct TriggerOTAUseCase: Sendable {
    private let repository: any OTARepository
    init(repository: any OTARepository) { self.repository = repository }
    func execute(deviceId: String, firmwareUpdateId: Int) async throws {
        try await repository.triggerOTA(deviceId: deviceId, firmwareUpdateId: firmwareUpdateId)
    }
}
