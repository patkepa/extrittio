import Foundation
import os

@Observable
@MainActor
final class AuthSession {
    private(set) var isAuthenticated = false
    private(set) var currentUser: User?
    var errorMessage: String?

    func restore(isAuthenticated: Bool, user: User?) {
        self.isAuthenticated = isAuthenticated
        self.currentUser = user
    }

    func authenticate(_ user: User) {
        isAuthenticated = true
        currentUser = user
        errorMessage = nil
    }

    func expire(message: String? = nil) {
        isAuthenticated = false
        currentUser = nil
        errorMessage = message
    }
}

@Observable
@MainActor
final class AuthViewModel {
    var isLoading = false

    var isAuthenticated: Bool { session.isAuthenticated }
    var currentUser: User? { session.currentUser }
    var errorMessage: String? {
        get { session.errorMessage }
        set { session.errorMessage = newValue }
    }

    private let loginUseCase: LoginUseCase
    private let logoutUseCase: LogoutUseCase
    private let checkAuthUseCase: CheckAuthUseCase
    private let cacheManager: CacheManager?
    private let session: AuthSession
    private let logger = Logger(subsystem: "com.extrittio", category: "Auth")

    init(
        loginUseCase: LoginUseCase,
        logoutUseCase: LogoutUseCase,
        checkAuthUseCase: CheckAuthUseCase,
        cacheManager: CacheManager? = nil,
        session: AuthSession = AuthSession()
    ) {
        self.loginUseCase = loginUseCase
        self.logoutUseCase = logoutUseCase
        self.checkAuthUseCase = checkAuthUseCase
        self.cacheManager = cacheManager
        self.session = session
        let hasToken = checkAuthUseCase.hasToken()
        session.restore(
            isAuthenticated: hasToken,
            user: hasToken ? checkAuthUseCase.cachedUser() : nil
        )
    }

    func login(username: String, password: String) async {
        isLoading = true
        errorMessage = nil
        do {
            let response = try await loginUseCase.execute(username: username, password: password)
            await cacheManager?.clearAll()
            session.authenticate(response.user)
            logger.info("Login successful for \(username, privacy: .private(mask: .hash))")
        } catch {
            errorMessage = error.localizedDescription
            logger.error("Login failed: \(error)")
        }
        isLoading = false
    }

    func logout() async {
        logoutUseCase.execute()
        session.expire()
        await cacheManager?.clearAll()
        logger.info("User logged out")
    }

    func checkAuth() async {
        do {
            if let user = try await checkAuthUseCase.execute() {
                session.authenticate(user)
            } else {
                session.expire()
                await cacheManager?.clearAll()
            }
        } catch {
            session.restore(
                isAuthenticated: checkAuthUseCase.hasToken(),
                user: checkAuthUseCase.cachedUser()
            )
            logger.warning("Auth validation failed; preserving local session: \(error)")
        }
    }
}
