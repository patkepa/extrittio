import Foundation

final class UserRepositoryImpl: UserRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getUsers() async throws -> [User] {
        let server = await apiClient.serverAddress
        let response: PaginatedResponse<User> = try await apiClient.get(Endpoints.users(server))
        return response.data
    }

    func createUser(_ request: AdminCreateUserRequest) async throws -> User {
        let server = await apiClient.serverAddress
        return try await apiClient.post(Endpoints.users(server), body: request)
    }

    func deleteUser(id: Int) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.user(server, id: id))
    }

    func setUserRoles(id: Int, roleIds: [Int]) async throws -> User {
        let server = await apiClient.serverAddress
        return try await apiClient.put(Endpoints.userRoles(server, id: id), body: SetUserRolesRequest(roleIds: roleIds))
    }

    func changePassword(id: Int, password: String) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.putNoContent(Endpoints.userPassword(server, id: id), body: ChangePasswordRequest(password: password))
    }
}

final class RoleRepositoryImpl: RoleRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getRoles() async throws -> [Role] {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.roles(server))
    }

    func getPermissions() async throws -> [PermissionResponse] {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.rolePermissions(server))
    }

    func createRole(_ request: CreateRoleRequest) async throws -> Role {
        let server = await apiClient.serverAddress
        return try await apiClient.post(Endpoints.roles(server), body: request)
    }

    func updateRole(id: Int, _ request: UpdateRoleRequest) async throws -> Role {
        let server = await apiClient.serverAddress
        return try await apiClient.put(Endpoints.role(server, id: id), body: request)
    }

    func deleteRole(id: Int) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.role(server, id: id))
    }
}

final class ApiKeyRepositoryImpl: ApiKeyRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getApiKeys() async throws -> [ApiKey] {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.apiKeys(server))
    }

    func createApiKey(_ request: CreateApiKeyRequest) async throws -> CreateApiKeyResponse {
        let server = await apiClient.serverAddress
        return try await apiClient.post(Endpoints.apiKeys(server), body: request)
    }

    func deleteApiKey(id: Int) async throws {
        let server = await apiClient.serverAddress
        try await apiClient.delete(Endpoints.apiKey(server, id: id))
    }
}

final class CertificateRepositoryImpl: CertificateRepository, Sendable {
    private let apiClient: APIClient
    init(apiClient: APIClient) { self.apiClient = apiClient }

    func getCaCertificate() async throws -> CaCertificateResponse {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.caCertificate(server))
    }

    func getDeviceCertificate(deviceId: String) async throws -> DeviceCertificateResponse {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.deviceCertificate(server, id: deviceId))
    }

    func regenerateDeviceCertificate(deviceId: String) async throws -> DeviceCertificateResponse {
        let server = await apiClient.serverAddress
        return try await apiClient.post(Endpoints.deviceCertificateRegenerate(server, id: deviceId))
    }

    func getDeviceCertificateStatus(deviceId: String) async throws -> DeviceCertificateStatusResponse? {
        let server = await apiClient.serverAddress
        return try await apiClient.get(Endpoints.deviceCertificateStatus(server, id: deviceId))
    }
}
