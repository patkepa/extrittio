import Foundation

protocol ServerAddressProvider: Sendable {
    var serverAddress: String { get }
}

struct UserDefaultsServerAddressProvider: ServerAddressProvider {
    var serverAddress: String {
        UserDefaults.standard.string(forKey: "serverAddress") ?? ""
    }
}
