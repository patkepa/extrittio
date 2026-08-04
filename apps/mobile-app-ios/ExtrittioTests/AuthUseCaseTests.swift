import Foundation
import SwiftData
import Testing
@testable import Extrittio

@Suite("Auth use cases")
struct AuthUseCaseTests {
    @Test("checkAuth keeps token and returns cached user on network failure")
    func checkAuthPreservesSessionOnNetworkFailure() async throws {
        let cachedUser = makeUser(username: "cached-user")
        let repository = MockAuthRepository(
            token: "existing-token",
            cachedUser: cachedUser,
            currentUserResult: .failure(APIError.networkError(URLError(.notConnectedToInternet)))
        )
        let useCase = CheckAuthUseCase(repository: repository)

        let user = try await useCase.execute()

        #expect(user?.username == "cached-user")
        #expect(repository.token == "existing-token")
        #expect(!repository.didDeleteToken)
        #expect(!repository.didDeleteCurrentUser)
    }

    @Test("checkAuth clears local auth on unauthorized response")
    func checkAuthClearsAuthOnUnauthorized() async throws {
        let repository = MockAuthRepository(
            token: "expired-token",
            cachedUser: makeUser(),
            currentUserResult: .failure(APIError.unauthorized)
        )
        let useCase = CheckAuthUseCase(repository: repository)

        let user = try await useCase.execute()

        #expect(user == nil)
        #expect(repository.token == nil)
        #expect(repository.cachedUser == nil)
        #expect(repository.didDeleteToken)
        #expect(repository.didDeleteCurrentUser)
    }

    @Test("login persists token and current user without retaining the password")
    func loginPersistsSessionData() async throws {
        let user = makeUser(username: "fresh-user")
        let repository = MockAuthRepository(loginResponse: LoginResponse(token: "new-token", user: user))
        let useCase = LoginUseCase(repository: repository)

        let response = try await useCase.execute(username: "fresh-user", password: "secret")

        #expect(response.token == "new-token")
        #expect(repository.token == "new-token")
        #expect(repository.cachedUser?.username == "fresh-user")
    }

    @Test("successful login clears data cached by a previous account")
    @MainActor
    func loginClearsCache() async {
        let repository = MockAuthRepository(
            loginResponse: LoginResponse(token: "new-token", user: makeUser(username: "fresh-user"))
        )
        let cacheManager = makeCacheManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        await cacheManager.cacheDevice(device)
        let viewModel = makeAuthViewModel(repository: repository, cacheManager: cacheManager)

        await viewModel.login(username: "fresh-user", password: "secret")

        #expect(viewModel.isAuthenticated)
        #expect(viewModel.currentUser?.username == "fresh-user")
        #expect(await cacheManager.getDevice(id: device.id) == nil)
    }

    @Test("logout clears authentication and cached account data")
    @MainActor
    func logoutClearsSessionAndCache() async {
        let repository = MockAuthRepository(
            token: "existing-token",
            cachedUser: makeUser(username: "current-user")
        )
        let cacheManager = makeCacheManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        await cacheManager.cacheDevice(device)
        let viewModel = makeAuthViewModel(repository: repository, cacheManager: cacheManager)

        await viewModel.logout()

        #expect(!viewModel.isAuthenticated)
        #expect(viewModel.currentUser == nil)
        #expect(repository.token == nil)
        #expect(repository.cachedUser == nil)
        #expect(await cacheManager.getDevice(id: device.id) == nil)
    }

    @Test("unauthorized auth check clears cached account data")
    @MainActor
    func unauthorizedCheckClearsCache() async {
        let repository = MockAuthRepository(
            token: "expired-token",
            cachedUser: makeUser(username: "expired-user"),
            currentUserResult: .failure(APIError.unauthorized)
        )
        let cacheManager = makeCacheManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        await cacheManager.cacheDevice(device)
        let viewModel = makeAuthViewModel(repository: repository, cacheManager: cacheManager)

        await viewModel.checkAuth()

        #expect(!viewModel.isAuthenticated)
        #expect(await cacheManager.getDevice(id: device.id) == nil)
    }
}

@MainActor
private func makeAuthViewModel(
    repository: MockAuthRepository,
    cacheManager: CacheManager
) -> AuthViewModel {
    AuthViewModel(
        loginUseCase: LoginUseCase(repository: repository),
        logoutUseCase: LogoutUseCase(repository: repository),
        checkAuthUseCase: CheckAuthUseCase(repository: repository),
        cacheManager: cacheManager
    )
}

private func makeCacheManager() -> CacheManager {
    let schema = Schema([
        CachedDevice.self, CachedDashboardStats.self, CachedAlertSummary.self,
        CachedTelemetryRecord.self, CachedDeviceShadow.self, CachedCommandRecord.self,
        CachedDeviceConfig.self, CachedDeviceLog.self, CachedAlert.self,
        CachedOtaDeployment.self, CachedFirmwareUpdate.self, CachedFleet.self,
        CachedDeviceType.self, CachedDeviceLocation.self, CachedMetrics.self,
        CachedRule.self, CachedZone.self, CacheEntry.self,
    ])
    let config = ModelConfiguration(isStoredInMemoryOnly: true)
    let container = try! ModelContainer(for: schema, configurations: [config])
    return CacheManager(modelContainer: container)
}

private func makeUser(username: String = "test-user") -> User {
    User(
        id: 1,
        username: username,
        role: "admin",
        roles: nil,
        permissions: ["server_metrics.read", "devices.read"],
        permissionVersion: 1
    )
}

private final class MockAuthRepository: AuthRepository, @unchecked Sendable {
    var loginResponse: LoginResponse
    var currentUserResult: Result<User, Error>
    var token: String?
    var cachedUser: User?
    var didDeleteToken = false
    var didDeleteCurrentUser = false

    init(
        token: String? = nil,
        cachedUser: User? = nil,
        currentUserResult: Result<User, Error> = .success(makeUser()),
        loginResponse: LoginResponse = LoginResponse(token: "token", user: makeUser())
    ) {
        self.token = token
        self.cachedUser = cachedUser
        self.currentUserResult = currentUserResult
        self.loginResponse = loginResponse
    }

    func login(username: String, password: String) async throws -> LoginResponse {
        loginResponse
    }

    func getCurrentUser() async throws -> User {
        try currentUserResult.get()
    }

    func saveToken(_ token: String) {
        self.token = token
    }

    func getToken() -> String? {
        token
    }

    func deleteToken() {
        didDeleteToken = true
        token = nil
    }

    func saveCurrentUser(_ user: User) {
        cachedUser = user
    }

    func getCachedCurrentUser() -> User? {
        cachedUser
    }

    func deleteCurrentUser() {
        didDeleteCurrentUser = true
        cachedUser = nil
    }

}
