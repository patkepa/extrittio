import Foundation

struct Role: Codable, Sendable, Identifiable, Hashable {
    let id: Int
    let name: String
    let description: String?
    let isSystem: Bool
    let permissions: [String]
    let userCount: Int

    enum CodingKeys: String, CodingKey {
        case id, name, description, permissions
        case isSystem = "is_system"
        case userCount = "user_count"
    }
}

struct PermissionResponse: Codable, Sendable, Identifiable, Hashable {
    let key: String
    var id: String { key }
}

struct AdminCreateUserRequest: Codable, Sendable {
    let username: String
    let password: String
    let roleIds: [Int]?

    enum CodingKeys: String, CodingKey {
        case username, password
        case roleIds = "role_ids"
    }
}

struct SetUserRolesRequest: Codable, Sendable {
    let roleIds: [Int]

    enum CodingKeys: String, CodingKey {
        case roleIds = "role_ids"
    }
}

struct ChangePasswordRequest: Codable, Sendable {
    let password: String
}

struct CreateRoleRequest: Codable, Sendable {
    let name: String
    let description: String?
    let permissions: [String]
}

struct UpdateRoleRequest: Codable, Sendable {
    let name: String?
    let description: String?
    let permissions: [String]?
}

struct ApiKey: Codable, Sendable, Identifiable, Hashable {
    let id: Int
    let name: String
    let keyPrefix: String
    let deviceTypeId: Int?
    let deviceTypeName: String?
    let createdAt: String
    let lastUsedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, name
        case keyPrefix = "key_prefix"
        case deviceTypeId = "device_type_id"
        case deviceTypeName = "device_type_name"
        case createdAt = "created_at"
        case lastUsedAt = "last_used_at"
    }
}

struct CreateApiKeyRequest: Codable, Sendable {
    let name: String
    let deviceTypeId: Int?

    enum CodingKeys: String, CodingKey {
        case name
        case deviceTypeId = "device_type_id"
    }
}

struct CreateApiKeyResponse: Codable, Sendable, Identifiable {
    let id: Int
    let name: String
    let key: String
    let keyPrefix: String
    let deviceTypeId: Int?

    enum CodingKeys: String, CodingKey {
        case id, name, key
        case keyPrefix = "key_prefix"
        case deviceTypeId = "device_type_id"
    }
}

struct CaCertificateResponse: Codable, Sendable {
    let fingerprint: String
    let certificatePem: String
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case fingerprint
        case certificatePem = "certificate_pem"
        case createdAt = "created_at"
    }
}

struct DeviceCertificateResponse: Codable, Sendable, Identifiable {
    let certificatePem: String
    let privateKeyPem: String
    let caPem: String
    let fingerprint: String
    let expiresAt: String
    let createdAt: String
    var id: String { fingerprint }

    enum CodingKeys: String, CodingKey {
        case fingerprint
        case certificatePem = "certificate_pem"
        case privateKeyPem = "private_key_pem"
        case caPem = "ca_pem"
        case expiresAt = "expires_at"
        case createdAt = "created_at"
    }
}

struct DeviceCertificateStatusResponse: Codable, Sendable, Hashable {
    let fingerprint: String
    let expiresAt: String
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case fingerprint
        case expiresAt = "expires_at"
        case createdAt = "created_at"
    }
}

struct CreateDeviceTypeRequest: Codable, Sendable {
    let name: String
    let icon: String?
    let colorHex: String?

    enum CodingKeys: String, CodingKey {
        case name, icon
        case colorHex = "color_hex"
    }
}

struct UpdateDeviceTypeRequest: Codable, Sendable {
    let name: String?
    let icon: String?
    let colorHex: String?

    enum CodingKeys: String, CodingKey {
        case name, icon
        case colorHex = "color_hex"
    }
}

struct CreateFleetRequest: Codable, Sendable {
    let name: String
}

struct UpdateFleetRequest: Codable, Sendable {
    let name: String
}
