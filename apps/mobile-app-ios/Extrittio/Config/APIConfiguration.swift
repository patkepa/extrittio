import Foundation

struct APIConfiguration: Equatable, Sendable {
    let scheme: String
    let port: Int?
    let basePath: String
    let allowsInsecureHTTP: Bool

    init(
        scheme: String = "https",
        port: Int? = nil,
        basePath: String = "/api/v1",
        allowsInsecureHTTP: Bool = false
    ) {
        let trimmedScheme = scheme.trimmingCharacters(in: .whitespacesAndNewlines)
        self.scheme = (trimmedScheme.isEmpty ? "https" : trimmedScheme).lowercased()
        self.port = port
        self.basePath = APIConfiguration.normalizedBasePath(basePath)
        self.allowsInsecureHTTP = allowsInsecureHTTP
    }

    static var current: APIConfiguration {
        let info = Bundle.main.infoDictionary ?? [:]
        let scheme = info["API_SCHEME"] as? String ?? "https"
        let portValue = (info["API_PORT"] as? String)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let port = portValue.flatMap { $0.isEmpty ? nil : Int($0) }
        let basePath = info["API_BASE_PATH"] as? String ?? "/api/v1"
        let allowsInsecureHTTP = boolValue(info["API_ALLOWS_INSECURE_HTTP"])
        return APIConfiguration(
            scheme: scheme,
            port: port,
            basePath: basePath,
            allowsInsecureHTTP: allowsInsecureHTTP
        )
    }

    func baseURL(serverAddress: String) -> URL? {
        url(serverAddress: serverAddress, endpointPath: "")
    }

    func healthURL(serverAddress: String) -> URL? {
        url(serverAddress: serverAddress, endpointPath: "health")
    }

    func url(
        serverAddress: String,
        endpointPath: String,
        queryItems: [URLQueryItem] = []
    ) -> URL? {
        let trimmedAddress = serverAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedAddress.isEmpty else { return nil }

        let addressWithScheme = trimmedAddress.contains("://")
            ? trimmedAddress
            : "\(scheme)://\(trimmedAddress)"

        guard var components = URLComponents(string: addressWithScheme) else {
            return nil
        }

        guard let resolvedScheme = components.scheme?.lowercased(),
              ["http", "https"].contains(resolvedScheme),
              resolvedScheme != "http" || allowsInsecureHTTP,
              components.host?.isEmpty == false,
              components.user == nil,
              components.password == nil else { return nil }

        components.scheme = resolvedScheme
        if components.port == nil {
            components.port = port
        }

        components.path = fullPath(endpointPath)
        components.queryItems = queryItems.isEmpty ? nil : queryItems
        return components.url
    }

    private func fullPath(_ endpointPath: String) -> String {
        let base = basePath.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let endpoint = endpointPath.trimmingCharacters(in: CharacterSet(charactersIn: "/"))

        switch (base.isEmpty, endpoint.isEmpty) {
        case (true, true):
            return "/"
        case (false, true):
            return "/\(base)"
        case (true, false):
            return "/\(endpoint)"
        case (false, false):
            return "/\(base)/\(endpoint)"
        }
    }

    private static func normalizedBasePath(_ path: String) -> String {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        return trimmed.hasPrefix("/") ? trimmed : "/\(trimmed)"
    }

    private static func boolValue(_ value: Any?) -> Bool {
        if let value = value as? Bool {
            return value
        }
        guard let value = value as? String else { return false }
        return ["1", "true", "yes"].contains(value.lowercased())
    }
}
