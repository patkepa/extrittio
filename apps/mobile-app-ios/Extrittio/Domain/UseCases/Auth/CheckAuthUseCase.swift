import Foundation

struct CheckAuthUseCase: Sendable {
    private let repository: any AuthRepository
    init(repository: any AuthRepository) { self.repository = repository }

    func execute() async throws -> User? {
        guard repository.getToken() != nil else { return nil }
        do {
            let user = try await repository.getCurrentUser()
            repository.saveCurrentUser(user)
            return user
        } catch APIError.unauthorized {
            repository.deleteToken()
            repository.deleteCurrentUser()
            return nil
        } catch {
            if let cachedUser = repository.getCachedCurrentUser() {
                return cachedUser
            }
            throw error
        }
    }

    func hasToken() -> Bool {
        repository.getToken() != nil
    }

    func cachedUser() -> User? {
        repository.getCachedCurrentUser()
    }
}
