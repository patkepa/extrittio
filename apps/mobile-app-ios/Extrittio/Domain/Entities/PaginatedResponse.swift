import Foundation

struct PaginatedResponse<T: Codable & Sendable>: Codable, Sendable {
    let data: [T]
    let total: Int
    let limit: Int
    let offset: Int
}
