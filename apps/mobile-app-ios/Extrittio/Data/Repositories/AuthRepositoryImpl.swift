import Foundation

final class AuthRepositoryImpl: AuthRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) {
        self.apiClient = apiClient
        KeychainHelper.deleteLegacyCredentials()
    }

    func login(username: String, password: String) async throws -> LoginResponse {
        let server = await apiClient.serverAddress
        let body = LoginRequest(username: username, password: password)
        return try await apiClient.postUnauthenticated(Endpoints.login(server), body: body)
    }

    func getCurrentUser() async throws -> User {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.me(server))
    }

    func saveToken(_ token: String) { KeychainHelper.saveToken(token) }
    func getToken() -> String? { KeychainHelper.getToken() }
    func deleteToken() { KeychainHelper.deleteToken() }
    func saveCurrentUser(_ user: User) { KeychainHelper.saveCurrentUser(user) }
    func getCachedCurrentUser() -> User? { KeychainHelper.getCurrentUser() }
    func deleteCurrentUser() { KeychainHelper.deleteCurrentUser() }
}
