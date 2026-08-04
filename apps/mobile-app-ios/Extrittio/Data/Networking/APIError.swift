import Foundation

enum APIError: Error, LocalizedError, Sendable {
    case invalidURL
    case invalidServerAddress
    case unauthorized
    case notFound
    case serverError(statusCode: Int, message: String?)
    case networkError(Error)
    case decodingError(Error)
    case noData

    var errorDescription: String? {
        switch self {
        case .invalidURL:
            "Invalid URL"
        case .invalidServerAddress:
            "Server address not configured. Go to Settings to set your server IP."
        case .unauthorized:
            "Session expired. Please log in again."
        case .notFound:
            "Resource not found."
        case .serverError(let code, let message):
            message ?? "Server error (\(code))"
        case .networkError(let error):
            "Network error: \(error.localizedDescription)"
        case .decodingError:
            "Failed to parse server response."
        case .noData:
            "No data received."
        }
    }
}

extension APIError: Equatable {
    static func == (lhs: APIError, rhs: APIError) -> Bool {
        switch (lhs, rhs) {
        case (.notFound, .notFound): true
        case (.unauthorized, .unauthorized): true
        case (.invalidURL, .invalidURL): true
        case (.invalidServerAddress, .invalidServerAddress): true
        case (.noData, .noData): true
        default: false
        }
    }
}
