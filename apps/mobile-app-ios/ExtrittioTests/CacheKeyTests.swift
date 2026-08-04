// ExtrittioTests/CacheKeyTests.swift
import Foundation
import Testing
@testable import Extrittio

@Suite("CacheKey")
struct CacheKeyTests {

    @Test("stringValue produces unique keys for each case")
    func uniqueStringValues() {
        let keys: [CacheKey] = [
            .devices, .device(id: "abc"), .dashboardStats, .alertSummary,
            .telemetry(deviceId: "abc"), .shadow(deviceId: "abc"),
            .commands(deviceId: "abc"), .config(deviceId: "abc"),
            .logs(deviceId: "abc"), .alerts(deviceId: "abc"),
            .otaDeployments(deviceId: "abc"), .firmware, .fleets,
            .deviceTypes, .location(deviceId: "abc"), .metrics,
            .metricsHistory, .rules, .zones
        ]
        let strings = keys.map { $0.stringValue }
        #expect(Set(strings).count == strings.count, "All stringValues must be unique")
    }

    @Test("device keys with different IDs produce different strings")
    func differentDeviceIds() {
        let key1 = CacheKey.device(id: "abc").stringValue
        let key2 = CacheKey.device(id: "xyz").stringValue
        #expect(key1 != key2)
    }

    @Test("same key produces same string")
    func sameKeySameString() {
        let key1 = CacheKey.telemetry(deviceId: "dev-1").stringValue
        let key2 = CacheKey.telemetry(deviceId: "dev-1").stringValue
        #expect(key1 == key2)
    }
}
