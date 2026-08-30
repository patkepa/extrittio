import Foundation
import Testing
@testable import Extrittio

@Suite("App deep links")
struct AppDeepLinkTests {
    @Test("nearby pairing link round-trips")
    func nearbyPairing() {
        let deepLink = AppDeepLink.pairNearbyDevice

        #expect(AppDeepLink(url: deepLink.url) == deepLink)
    }

    @Test("unrelated and malformed links are rejected", arguments: [
        "https://extrittio.com/pair-nearby",
        "extrittio://devices",
        "extrittio://pair-nearby/extra",
        "extrittio://pair-nearby?device=one"
    ])
    func invalidLink(rawValue: String) throws {
        let url = try #require(URL(string: rawValue))

        #expect(AppDeepLink(url: url) == nil)
    }

    @Test("router publishes and consumes the nearby pairing destination")
    @MainActor
    func routerRequest() throws {
        let router = AppNavigationRouter()

        #expect(router.handle(url: AppDeepLink.pairNearbyDevice.url))
        let request = try #require(router.request)
        #expect(request.destination == .pairNearbyDevice)

        router.consume(request)
        #expect(router.request == nil)
    }

    @Test("repeated pairing links publish fresh requests")
    @MainActor
    func repeatedPairingRequests() throws {
        let router = AppNavigationRouter()

        #expect(router.handle(url: AppDeepLink.pairNearbyDevice.url))
        let firstRequest = try #require(router.request)

        #expect(router.handle(url: AppDeepLink.pairNearbyDevice.url))
        let secondRequest = try #require(router.request)

        #expect(firstRequest.id != secondRequest.id)
        #expect(secondRequest.destination == .pairNearbyDevice)
    }
}
