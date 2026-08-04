import Foundation
import Testing
@testable import Extrittio

@Suite("ViewState")
struct ViewStateTests {

    @Test("isLoading returns true only for loading case")
    func isLoading() {
        let loading: ViewState<String> = .loading
        let loaded: ViewState<String> = .loaded("data")
        let error: ViewState<String> = .error(TestError.sample)
        let empty: ViewState<String> = .empty
        let loadingMore: ViewState<String> = .loadingMore("data")

        #expect(loading.isLoading == true)
        #expect(loaded.isLoading == false)
        #expect(error.isLoading == false)
        #expect(empty.isLoading == false)
        #expect(loadingMore.isLoading == false)
    }

    @Test("data returns value for loaded and loadingMore cases")
    func data() {
        let loading: ViewState<String> = .loading
        let loaded: ViewState<String> = .loaded("hello")
        let loadingMore: ViewState<String> = .loadingMore("world")
        let error: ViewState<String> = .error(TestError.sample)

        #expect(loading.data == nil)
        #expect(loaded.data == "hello")
        #expect(loadingMore.data == "world")
        #expect(error.data == nil)
    }

    @Test("errorMessage returns description for error case")
    func errorMessage() {
        let error: ViewState<String> = .error(TestError.sample)
        let loaded: ViewState<String> = .loaded("data")

        #expect(error.errorMessage != nil)
        #expect(loaded.errorMessage == nil)
    }

    @Test("cached case returns data and cacheDate")
    func cachedCase() {
        let date = Date()
        let cached: ViewState<String> = .cached("stale data", lastUpdated: date)

        #expect(cached.data == "stale data")
        #expect(cached.isCached == true)
        #expect(cached.cacheDate == date)
        #expect(cached.isLoading == false)
        #expect(cached.errorMessage == nil)
    }

    @Test("isCached returns false for non-cached cases")
    func isCachedFalse() {
        let loaded: ViewState<String> = .loaded("fresh")
        let loading: ViewState<String> = .loading

        #expect(loaded.isCached == false)
        #expect(loading.isCached == false)
    }

    enum TestError: Error, LocalizedError {
        case sample
        var errorDescription: String? { "Sample error" }
    }
}
