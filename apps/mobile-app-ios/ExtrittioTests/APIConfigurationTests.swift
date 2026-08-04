import Foundation
import Testing
@testable import Extrittio

@Suite("APIConfiguration")
struct APIConfigurationTests {
    @Test("builds base URL from host and configured components")
    func baseURLFromHost() {
        let config = APIConfiguration(scheme: "https", port: 8443, basePath: "/api/v2")
        #expect(config.baseURL(serverAddress: "example.com")?.absoluteString == "https://example.com:8443/api/v2")
    }

    @Test("keeps explicit URL scheme and port from user input")
    func explicitURLInputWins() {
        let config = APIConfiguration(
            scheme: "https",
            port: nil,
            basePath: "api/v1",
            allowsInsecureHTTP: true
        )
        #expect(config.healthURL(serverAddress: "http://localhost:8080")?.absoluteString == "http://localhost:8080/api/v1/health")
    }

    @Test("rejects explicit HTTP when insecure connections are disabled")
    func rejectsInsecureHTTP() {
        let config = APIConfiguration(
            scheme: "https",
            port: nil,
            basePath: "/api/v1",
            allowsInsecureHTTP: false
        )

        #expect(config.baseURL(serverAddress: "http://example.com") == nil)
    }

    @Test("rejects unsupported schemes and URLs containing credentials")
    func rejectsUnsupportedURLs() {
        let config = APIConfiguration(
            scheme: "https",
            port: nil,
            basePath: "/api/v1",
            allowsInsecureHTTP: true
        )

        #expect(config.baseURL(serverAddress: "ftp://example.com") == nil)
        #expect(config.baseURL(serverAddress: "https://user:password@example.com") == nil)
    }

    @Test("adds query items using URLComponents")
    func queryItems() {
        let config = APIConfiguration(scheme: "https", port: nil, basePath: "/api/v1")
        let url = config.url(
            serverAddress: "example.com",
            endpointPath: "server/metrics/history",
            queryItems: [URLQueryItem(name: "since", value: "2026-05-16T00:00:00")]
        )
        #expect(url?.absoluteString == "https://example.com/api/v1/server/metrics/history?since=2026-05-16T00:00:00")
    }
}
