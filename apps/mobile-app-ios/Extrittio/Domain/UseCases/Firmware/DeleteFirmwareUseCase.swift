import Foundation

struct DeleteFirmwareUseCase: Sendable {
    private let repository: any FirmwareRepository
    init(repository: any FirmwareRepository) { self.repository = repository }
    func execute(id: Int) async throws { try await repository.deleteFirmware(id: id) }
}
