import Foundation

struct LoginRequest: Codable, Sendable {
    let username: String
    let password: String
    /// Native clients authenticate subsequent API calls with a bearer token.
    let issueToken = true

    enum CodingKeys: String, CodingKey {
        case username
        case password
        case issueToken = "issue_token"
    }
}

struct LoginResponse: Codable, Sendable {
    let token: String
    let user: User
}
