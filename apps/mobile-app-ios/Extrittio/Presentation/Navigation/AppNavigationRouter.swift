import Foundation
import Observation

@Observable
@MainActor
final class AppNavigationRouter {
    struct Request: Equatable, Identifiable {
        let id = UUID()
        let destination: Destination
    }

    enum Destination: Equatable {
        case pairNearbyDevice
    }

    private(set) var request: Request?

    @discardableResult
    func handle(url: URL) -> Bool {
        guard let deepLink = AppDeepLink(url: url) else { return false }

        switch deepLink {
        case .pairNearbyDevice:
            open(.pairNearbyDevice)
        }
        return true
    }

    func open(_ destination: Destination) {
        request = Request(destination: destination)
    }

    func consume(_ handledRequest: Request) {
        guard request?.id == handledRequest.id else { return }
        request = nil
    }
}
