import Foundation

struct LogoutUseCase: Sendable {
    private let repository: any AuthRepository
    init(repository: any AuthRepository) { self.repository = repository }

    func execute() {
        repository.deleteToken()
        repository.deleteCurrentUser()
    }
}
