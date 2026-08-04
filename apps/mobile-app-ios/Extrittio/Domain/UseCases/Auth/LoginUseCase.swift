import Foundation

struct LoginUseCase: Sendable {
    private let repository: any AuthRepository
    init(repository: any AuthRepository) { self.repository = repository }

    func execute(username: String, password: String) async throws -> LoginResponse {
        let response = try await repository.login(username: username, password: password)
        repository.saveToken(response.token)
        repository.saveCurrentUser(response.user)
        return response
    }
}
