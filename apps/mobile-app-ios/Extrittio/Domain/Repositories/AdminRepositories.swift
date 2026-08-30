import Foundation

protocol UserRepository: Sendable {
    func getUsers() async throws -> [User]
    func createUser(_ request: AdminCreateUserRequest) async throws -> User
    func deleteUser(id: Int) async throws
    func setUserRoles(id: Int, roleIds: [Int]) async throws -> User
    func changePassword(id: Int, password: String) async throws
}

protocol RoleRepository: Sendable {
    func getRoles() async throws -> [Role]
    func getPermissions() async throws -> [PermissionResponse]
    func createRole(_ request: CreateRoleRequest) async throws -> Role
    func updateRole(id: Int, _ request: UpdateRoleRequest) async throws -> Role
    func deleteRole(id: Int) async throws
}

protocol ApiKeyRepository: Sendable {
    func getApiKeys() async throws -> [ApiKey]
    func createApiKey(_ request: CreateApiKeyRequest) async throws -> CreateApiKeyResponse
    func deleteApiKey(id: Int) async throws
}

protocol CertificateRepository: Sendable {
    func getCaCertificate() async throws -> CaCertificateResponse
    func getDeviceCertificate(deviceId: String) async throws -> DeviceCertificateResponse
    func regenerateDeviceCertificate(deviceId: String) async throws -> DeviceCertificateResponse
    func getDeviceCertificateStatus(deviceId: String) async throws -> DeviceCertificateStatusResponse?
}

protocol ThreadDatasetRepository: Sendable {
    func getActiveDataset() async throws -> DeviceProvisioningThread
}
