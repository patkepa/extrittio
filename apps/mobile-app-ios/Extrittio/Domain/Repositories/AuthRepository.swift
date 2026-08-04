import Foundation

protocol AuthRepository: Sendable {
    func login(username: String, password: String) async throws -> LoginResponse
    func getCurrentUser() async throws -> User
    func saveToken(_ token: String)
    func getToken() -> String?
    func deleteToken()
    func saveCurrentUser(_ user: User)
    func getCachedCurrentUser() -> User?
    func deleteCurrentUser()
}
