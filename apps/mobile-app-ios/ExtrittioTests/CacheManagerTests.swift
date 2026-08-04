// ExtrittioTests/CacheManagerTests.swift
import Foundation
import Testing
import SwiftData
@testable import Extrittio

@Suite("CacheManager")
struct CacheManagerTests {

    private func makeManager() -> CacheManager {
        let schema = Schema([
            CachedDevice.self, CachedDashboardStats.self, CachedAlertSummary.self,
            CachedTelemetryRecord.self, CachedDeviceShadow.self, CachedCommandRecord.self,
            CachedDeviceConfig.self, CachedDeviceLog.self, CachedAlert.self,
            CachedOtaDeployment.self, CachedFirmwareUpdate.self, CachedFleet.self,
            CachedDeviceType.self, CachedDeviceLocation.self, CachedMetrics.self,
            CachedRule.self, CachedZone.self, CacheEntry.self
        ])
        let config = ModelConfiguration(isStoredInMemoryOnly: true)
        let container = try! ModelContainer(for: schema, configurations: [config])
        return CacheManager(modelContainer: container)
    }

    @Test("cache and retrieve a device")
    func cacheDevice() async {
        let manager = makeManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        await manager.cacheDevice(device)
        let result = await manager.getDevice(id: device.id)
        #expect(result != nil)
        #expect(result?.id == device.id)
        #expect(result?.name == device.name)
    }

    @Test("returns nil for uncached device")
    func uncachedDevice() async {
        let manager = makeManager()
        let result = await manager.getDevice(id: "nonexistent")
        #expect(result == nil)
    }

    @Test("cache and retrieve dashboard stats")
    func cacheDashboardStats() async {
        let manager = makeManager()
        let stats: DashboardStats = TestData.decode(TestData.dashboardStatsJSON)
        await manager.cacheDashboardStats(stats)
        let result = await manager.getDashboardStats()
        #expect(result != nil)
        #expect(result?.totalDevices == stats.totalDevices)
    }

    @Test("lastUpdated returns date after caching")
    func lastUpdated() async {
        let manager = makeManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        let before = await manager.lastUpdated(for: .device(id: device.id))
        #expect(before == nil)
        await manager.cacheDevice(device)
        let after = await manager.lastUpdated(for: .device(id: device.id))
        #expect(after != nil)
    }

    @Test("clearAll removes all cached data")
    func clearAll() async {
        let manager = makeManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        await manager.cacheDevice(device)
        #expect(await manager.getDevice(id: device.id) != nil)
        await manager.clearAll()
        #expect(await manager.getDevice(id: device.id) == nil)
    }

    @Test("evictExpired removes old entries")
    func evictExpired() async {
        let manager = makeManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        await manager.cacheDevice(device)
        await manager.evictExpired(maxAge: 0)
        #expect(await manager.getDevice(id: device.id) == nil)
    }

    @Test("removeDevice evicts from cache")
    func removeDevice() async {
        let manager = makeManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        await manager.cacheDevice(device)
        #expect(await manager.getDevice(id: device.id) != nil)
        await manager.removeDevice(id: device.id)
        #expect(await manager.getDevice(id: device.id) == nil)
    }

    @Test("evictIfOverSize removes oldest entries when over limit")
    func evictIfOverSize() async {
        let manager = makeManager()
        let device: Device = TestData.decode(TestData.deviceJSON)
        let stats: DashboardStats = TestData.decode(TestData.dashboardStatsJSON)
        await manager.cacheDevice(device)
        await manager.cacheDashboardStats(stats)
        await manager.evictIfOverSize(maxBytes: 1)
        #expect(await manager.getDevice(id: device.id) == nil)
        #expect(await manager.getDashboardStats() == nil)
    }
}
