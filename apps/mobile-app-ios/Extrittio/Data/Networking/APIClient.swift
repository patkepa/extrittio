import Foundation
import os

actor APIClient {
    private let logger = Logger(subsystem: "com.extrittio", category: "APIClient")
    private let session: URLSession
    private let decoder: JSONDecoder
    private let serverAddressProvider: ServerAddressProvider
    private let unauthorizedHandler: @Sendable () async -> Void

    init(
        serverAddressProvider: ServerAddressProvider,
        unauthorizedHandler: @escaping @Sendable () async -> Void = {}
    ) {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 15
        self.session = URLSession(configuration: config)
        self.decoder = JSONDecoder()
        self.serverAddressProvider = serverAddressProvider
        self.unauthorizedHandler = unauthorizedHandler
    }

    var serverAddress: String {
        serverAddressProvider.serverAddress
    }

    private func makeRequest(url urlString: String, method: String = "GET", body: (any Encodable & Sendable)? = nil, authenticated: Bool = true) async throws(APIError) -> Data {
        guard !serverAddress.isEmpty else { throw .invalidServerAddress }
        guard let url = URL(string: urlString),
              let scheme = url.scheme?.lowercased(),
              ["http", "https"].contains(scheme),
              url.host != nil else { throw .invalidURL }

        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        if authenticated {
            guard let token = KeychainHelper.getToken() else { throw .unauthorized }
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        if let body {
            do {
                request.httpBody = try JSONEncoder().encode(body)
            } catch {
                throw .decodingError(error)
            }
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw .networkError(error)
        }

        guard let httpResponse = response as? HTTPURLResponse else { throw .noData }

        switch httpResponse.statusCode {
        case 200...201:
            return data
        case 204:
            return Data()
        case 401:
            if authenticated {
                KeychainHelper.deleteToken()
                KeychainHelper.deleteCurrentUser()
                await unauthorizedHandler()
            }
            throw .unauthorized
        case 404:
            throw .notFound
        default:
            let message = String(data: data, encoding: .utf8)
            throw .serverError(statusCode: httpResponse.statusCode, message: message)
        }
    }

    func get<T: Decodable & Sendable>(_ url: String) async throws(APIError) -> T {
        let data = try await makeRequest(url: url)
        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            logger.error("Decode error for \(url): \(error)")
            throw .decodingError(error)
        }
    }

    func post<T: Decodable & Sendable>(_ url: String, body: (any Encodable & Sendable)? = nil) async throws(APIError) -> T {
        let data = try await makeRequest(url: url, method: "POST", body: body)
        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            logger.error("Decode error for \(url): \(error)")
            throw .decodingError(error)
        }
    }

    func postUnauthenticated<T: Decodable & Sendable>(_ url: String, body: (any Encodable & Sendable)? = nil) async throws(APIError) -> T {
        let data = try await makeRequest(url: url, method: "POST", body: body, authenticated: false)
        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            logger.error("Decode error for \(url): \(error)")
            throw .decodingError(error)
        }
    }

    func postNoContent(_ url: String, body: (any Encodable & Sendable)? = nil) async throws(APIError) {
        _ = try await makeRequest(url: url, method: "POST", body: body)
    }

    func put<T: Decodable & Sendable>(_ url: String, body: (any Encodable & Sendable)? = nil) async throws(APIError) -> T {
        let data = try await makeRequest(url: url, method: "PUT", body: body)
        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            logger.error("Decode error for \(url): \(error)")
            throw .decodingError(error)
        }
    }

    func putNoContent(_ url: String, body: (any Encodable & Sendable)? = nil) async throws(APIError) {
        _ = try await makeRequest(url: url, method: "PUT", body: body)
    }

    func patch<T: Decodable & Sendable>(_ url: String, body: (any Encodable & Sendable)? = nil) async throws(APIError) -> T {
        let data = try await makeRequest(url: url, method: "PATCH", body: body)
        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            logger.error("Decode error for \(url): \(error)")
            throw .decodingError(error)
        }
    }

    func delete(_ url: String) async throws(APIError) {
        _ = try await makeRequest(url: url, method: "DELETE")
    }

}
