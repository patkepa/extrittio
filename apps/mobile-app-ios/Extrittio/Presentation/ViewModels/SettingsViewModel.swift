import Foundation

@Observable
@MainActor
final class SettingsViewModel {
    var serverAddress: String {
        get { UserDefaults.standard.string(forKey: "serverAddress") ?? "" }
        set { UserDefaults.standard.set(newValue, forKey: "serverAddress") }
    }

    enum ConnectionTestResult {
        case success
        case failure(String)
    }

    var connectionTestResult: ConnectionTestResult?
    var isTesting = false
    var cacheSize: String = "Calculating..."
    var isClearing = false

    private let cacheManager: CacheManager

    init(cacheManager: CacheManager) {
        self.cacheManager = cacheManager
    }

    var appVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "Unknown"
    }

    var buildNumber: String {
        Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "Unknown"
    }

    func loadCacheSize() async {
        let bytes = await cacheManager.totalCacheSize()
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        cacheSize = formatter.string(fromByteCount: bytes)
    }

    func clearCache() async {
        isClearing = true
        await cacheManager.clearAll()
        await loadCacheSize()
        isClearing = false
    }

    func testConnection() async {
        isTesting = true
        connectionTestResult = nil

        guard !serverAddress.isEmpty else {
            connectionTestResult = .failure("No server address configured")
            isTesting = false
            return
        }

        guard let url = APIConfiguration.current.healthURL(serverAddress: serverAddress) else {
            connectionTestResult = .failure("Invalid server address")
            isTesting = false
            return
        }

        do {
            let config = URLSessionConfiguration.default
            config.timeoutIntervalForRequest = 5
            let session = URLSession(configuration: config)
            let (_, response) = try await session.data(from: url)
            if let http = response as? HTTPURLResponse, http.statusCode == 200 {
                connectionTestResult = .success
            } else {
                connectionTestResult = .failure("Server returned error")
            }
        } catch {
            connectionTestResult = .failure(error.localizedDescription)
        }
        isTesting = false
    }
}
