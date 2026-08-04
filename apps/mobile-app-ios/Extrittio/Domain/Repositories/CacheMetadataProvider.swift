// Extrittio/Domain/Repositories/CacheMetadataProvider.swift
import Foundation

protocol CacheMetadataProvider: Sendable {
    func lastUpdated(for key: CacheKey) async -> Date?
    func totalCacheSize() async -> Int64
    func clearAll() async
}
