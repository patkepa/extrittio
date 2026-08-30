import Foundation

enum AppDeepLink: Equatable, Sendable {
    case pairNearbyDevice

    static let scheme = "extrittio"

    var url: URL {
        switch self {
        case .pairNearbyDevice:
            URL(string: "\(Self.scheme)://pair-nearby")!
        }
    }

    init?(url: URL) {
        guard url.scheme?.lowercased() == Self.scheme,
              url.user == nil,
              url.password == nil,
              url.port == nil,
              url.query == nil,
              url.fragment == nil,
              url.path.isEmpty
        else {
            return nil
        }

        switch url.host?.lowercased() {
        case "pair-nearby": self = .pairNearbyDevice
        default: return nil
        }
    }
}
