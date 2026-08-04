import Foundation
import Security
import os

struct KeychainHelper: Sendable {
    private static let logger = Logger(subsystem: "com.extrittio", category: "Keychain")
    private static let service = "com.extrittio.auth"
    private static let tokenKey = "jwt_token"

    static func saveToken(_ token: String) {
        save(value: token, forKey: tokenKey)
    }

    static func getToken() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: tokenKey,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func deleteToken() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: tokenKey
        ]
        SecItemDelete(query as CFDictionary)
    }

    // MARK: - Current User

    private static let currentUserKey = "current_user"

    static func saveCurrentUser(_ user: User) {
        do {
            let data = try JSONEncoder().encode(user)
            save(data: data, forKey: currentUserKey)
        } catch {
            logger.error("Failed to encode current user: \(error)")
        }
    }

    static func getCurrentUser() -> User? {
        guard let data = getData(forKey: currentUserKey) else { return nil }
        do {
            return try JSONDecoder().decode(User.self, from: data)
        } catch {
            logger.error("Failed to decode current user: \(error)")
            return nil
        }
    }

    static func deleteCurrentUser() {
        deleteValue(forKey: currentUserKey)
    }

    static func deleteLegacyCredentials() {
        deleteValue(forKey: "credentials_username")
        deleteValue(forKey: "credentials_password")
    }

    // MARK: - Generic Keychain Helpers

    private static func save(value: String, forKey key: String) {
        guard let data = value.data(using: .utf8) else { return }
        save(data: data, forKey: key)
    }

    private static func save(data: Data, forKey key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key
        ]
        SecItemDelete(query as CFDictionary)
        var addQuery = query
        addQuery[kSecValueData as String] = data
        addQuery[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        let status = SecItemAdd(addQuery as CFDictionary, nil)
        if status != errSecSuccess {
            logger.error("Failed to save \(key): \(status)")
        }
    }

    private static func getData(forKey key: String) -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return data
    }

    private static func deleteValue(forKey key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key
        ]
        SecItemDelete(query as CFDictionary)
    }
}
