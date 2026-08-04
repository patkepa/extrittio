import Foundation

struct SendCommandRequest: Codable, Sendable {
    let command: String
    let params: [String: AnyCodableValue]?
}
