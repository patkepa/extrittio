import Testing
import Foundation
@testable import Extrittio

@Test func parseISO8601Timestamp() {
    let date = Date.fromISO8601("2025-01-15T10:30:00")
    #expect(date != nil)
}

@Test func parseISO8601InvalidReturnsNil() {
    let date = Date.fromISO8601("not-a-date")
    #expect(date == nil)
}

@Test func relativeTimeJustNow() {
    let date = Date.now.addingTimeInterval(-30)
    let result = date.relativeTimeString
    #expect(result == "Just now")
}

@Test func relativeTimeMinutesAgo() {
    let date = Date.now.addingTimeInterval(-120)
    let result = date.relativeTimeString
    #expect(result == "2m ago")
}

@Test func relativeTimeHoursAgo() {
    let date = Date.now.addingTimeInterval(-7200)
    let result = date.relativeTimeString
    #expect(result == "2h ago")
}

@Test func relativeTimeYesterday() {
    let date = Date.now.addingTimeInterval(-86400)
    let result = date.relativeTimeString
    #expect(result == "Yesterday")
}

@Test func relativeTimeOlderUsesShortFormat() {
    let date = Date.now.addingTimeInterval(-604800) // 7 days ago
    let result = date.relativeTimeString
    #expect(!result.contains("ago"))
    #expect(!result.isEmpty)
}

@Test func formatTimestampStringValid() {
    let result = String.formattedTimestamp("2025-01-15T10:30:00")
    #expect(result != "2025-01-15T10:30:00") // should be formatted
    #expect(!result.isEmpty)
}

@Test func formatTimestampStringInvalid() {
    let result = String.formattedTimestamp("bad")
    #expect(result == "bad") // falls back to original
}
