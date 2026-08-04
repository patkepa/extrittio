import Foundation
import os

enum ConnectionState: Equatable {
    case connected
    case reconnecting
    case disconnected
}

@Observable
@MainActor
final class ConnectionMonitor {
    var state: ConnectionState = .connected
    var isOnline: Bool { state == .connected }

    private let serverAddressProvider: ServerAddressProvider
    private let logger = Logger(subsystem: "com.extrittio", category: "ConnectionMonitor")
    private let session: URLSession

    init(serverAddressProvider: ServerAddressProvider) {
        self.serverAddressProvider = serverAddressProvider
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 5
        self.session = URLSession(configuration: config)
    }

    func startMonitoring() async {
        while !Task.isCancelled {
            await checkHealth()
            try? await Task.sleep(for: .seconds(30))
        }
    }

    func checkHealth() async {
        let address = serverAddressProvider.serverAddress
        guard !address.isEmpty else {
            state = .disconnected
            return
        }

        guard let url = APIConfiguration.current.healthURL(serverAddress: address) else {
            state = .disconnected
            return
        }

        if state == .disconnected {
            state = .reconnecting
        }

        do {
            let (_, response) = try await session.data(from: url)
            if let http = response as? HTTPURLResponse, http.statusCode == 200 {
                state = .connected
            } else {
                state = .disconnected
            }
        } catch {
            state = .disconnected
            logger.debug("Health check failed: \(error.localizedDescription)")
        }
    }
}
