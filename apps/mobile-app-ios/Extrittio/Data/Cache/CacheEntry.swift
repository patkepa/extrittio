// Extrittio/Data/Cache/CacheEntry.swift
import Foundation
import SwiftData

@Model
final class CacheEntry {
    @Attribute(.unique) var key: String
    var lastUpdated: Date
    var sizeBytes: Int

    init(key: String, lastUpdated: Date, sizeBytes: Int) {
        self.key = key
        self.lastUpdated = lastUpdated
        self.sizeBytes = sizeBytes
    }
}
