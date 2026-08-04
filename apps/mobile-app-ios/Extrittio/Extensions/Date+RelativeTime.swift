import Foundation

extension Date {
    static func fromISO8601(_ string: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: string) { return date }
        formatter.formatOptions = [.withInternetDateTime]
        if let date = formatter.date(from: string) { return date }
        // Backend sends NaiveDateTime: "2025-01-15T10:30:00"
        let naive = DateFormatter()
        naive.dateFormat = "yyyy-MM-dd'T'HH:mm:ss"
        naive.locale = Locale(identifier: "en_US_POSIX")
        return naive.date(from: string)
    }

    var relativeTimeString: String {
        let seconds = Int(-timeIntervalSinceNow)
        if seconds < 60 { return "Just now" }
        if seconds < 3600 { return "\(seconds / 60)m ago" }
        if seconds < 86400 { return "\(seconds / 3600)h ago" }
        if seconds < 172800 { return "Yesterday" }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: self)
    }
}

extension String {
    static func formattedTimestamp(_ iso: String) -> String {
        guard let date = Date.fromISO8601(iso) else { return iso }
        return date.relativeTimeString
    }
}
