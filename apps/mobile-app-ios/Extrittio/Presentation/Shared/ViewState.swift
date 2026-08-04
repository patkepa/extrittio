import Foundation

enum ViewState<T> {
    case loading
    case loaded(T)
    case cached(T, lastUpdated: Date)
    case loadingMore(T)
    case error(Error)
    case empty

    var isLoading: Bool {
        if case .loading = self { return true }
        return false
    }

    var data: T? {
        switch self {
        case .loaded(let value), .cached(let value, _), .loadingMore(let value): return value
        default: return nil
        }
    }

    var errorMessage: String? {
        if case .error(let error) = self {
            return error.localizedDescription
        }
        return nil
    }

    var isLoadingMore: Bool {
        if case .loadingMore = self { return true }
        return false
    }

    var isCached: Bool {
        if case .cached = self { return true }
        return false
    }

    var cacheDate: Date? {
        if case .cached(_, let date) = self { return date }
        return nil
    }
}
