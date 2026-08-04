import Foundation

struct LoginRequest: Codable, Sendable {
    let username: String
    let password: String
}

struct LoginResponse: Codable, Sendable {
    let token: String
    let user: User
}
