import Foundation
import Testing
@testable import Extrittio

@Suite("CachedModel Mapping")
struct CachedModelTests {

    @Test("CachedDevice round-trips through toDomain")
    func deviceRoundTrip() {
        let device: Device = TestData.decode(TestData.deviceJSON)
        let cached = CachedDevice.from(device)
        let result = cached.toDomain()

        #expect(result.id == device.id)
        #expect(result.name == device.name)
        #expect(result.deviceTypeId == device.deviceTypeId)
        #expect(result.deviceTypeName == device.deviceTypeName)
        #expect(result.fleetId == device.fleetId)
        #expect(result.fleetName == device.fleetName)
        #expect(result.status == device.status)
        #expect(result.firmware == device.firmware)
        #expect(result.lastSeen == device.lastSeen)
        #expect(result.lastSeenAt == device.lastSeenAt)
        #expect(result.uptime == device.uptime)
        #expect(result.uptimeSeconds == device.uptimeSeconds)
    }

    @Test("CachedDashboardStats round-trips through toDomain")
    func dashboardStatsRoundTrip() {
        let stats: DashboardStats = TestData.decode(TestData.dashboardStatsJSON)
        let cached = CachedDashboardStats.from(stats)
        let result = cached.toDomain()

        #expect(result.totalDevices == stats.totalDevices)
        #expect(result.activeDevices == stats.activeDevices)
        #expect(result.offlineDevices == stats.offlineDevices)
        #expect(result.totalMessages == stats.totalMessages)
    }

    @Test("CachedFleet round-trips through toDomain")
    func fleetRoundTrip() {
        let fleet = Fleet(id: 1, name: "Floor 1", deviceCount: 5)
        let cached = CachedFleet.from(fleet)
        let result = cached.toDomain()

        #expect(result.id == fleet.id)
        #expect(result.name == fleet.name)
        #expect(result.deviceCount == fleet.deviceCount)
    }

    @Test("CachedDeviceType round-trips through toDomain")
    func deviceTypeRoundTrip() {
        let dt = DeviceType(id: 1, name: "Temp Sensor")
        let cached = CachedDeviceType.from(dt)
        let result = cached.toDomain()

        #expect(result.id == dt.id)
        #expect(result.name == dt.name)
    }

    @Test("CachedTelemetryRecord round-trips through toDomain")
    func telemetryRoundTrip() {
        let records: [TelemetryRecord] = TestData.decode(TestData.telemetryJSON)
        let record = records[0]
        let cached = CachedTelemetryRecord.from(record)
        let result = cached.toDomain()

        #expect(result.id == record.id)
        #expect(result.deviceId == record.deviceId)
        #expect(result.temperature == record.temperature)
        #expect(result.humidity == record.humidity)
        #expect(result.batteryLevel == record.batteryLevel)
        #expect(result.receivedAt == record.receivedAt)
    }

    @Test("CachedDeviceShadow round-trips through toDomain")
    func shadowRoundTrip() {
        let shadow: DeviceShadow = TestData.decode(TestData.shadowJSON)
        let cached = CachedDeviceShadow.from(shadow)
        let result = cached.toDomain()

        #expect(result.deviceId == shadow.deviceId)
        #expect(result.version == shadow.version)
        #expect(result.updatedAt == shadow.updatedAt)
    }

    @Test("CachedCommandRecord round-trips through toDomain")
    func commandRoundTrip() {
        let cmd: CommandRecord = TestData.decode(TestData.commandJSON)
        let cached = CachedCommandRecord.from(cmd)
        let result = cached.toDomain()

        #expect(result.id == cmd.id)
        #expect(result.command == cmd.command)
        #expect(result.status == cmd.status)
    }

    @Test("CachedAlert round-trips through toDomain")
    func alertRoundTrip() {
        let alert: Alert = TestData.decode(TestData.alertJSON)
        let cached = CachedAlert.from(alert)
        let result = cached.toDomain()

        #expect(result.id == alert.id)
        #expect(result.severity == alert.severity)
        #expect(result.status == alert.status)
        #expect(result.message == alert.message)
        #expect(result.deviceId == alert.deviceId)
    }

    @Test("CachedFirmwareUpdate round-trips through toDomain")
    func firmwareRoundTrip() {
        let fw: FirmwareUpdate = TestData.decode(TestData.firmwareJSON)
        let cached = CachedFirmwareUpdate.from(fw)
        let result = cached.toDomain()

        #expect(result.id == fw.id)
        #expect(result.version == fw.version)
        #expect(result.deviceTypeId == fw.deviceTypeId)
    }

    @Test("CachedRule round-trips through toDomain preserving conditions/actions")
    func ruleRoundTrip() {
        let rule: Rule = TestData.decode(TestData.ruleJSON)
        let cached = CachedRule.from(rule)
        let result = cached.toDomain()

        #expect(result != nil)
        #expect(result?.id == rule.id)
        #expect(result?.name == rule.name)
        #expect(result?.enabled == rule.enabled)
        #expect(result?.conditions.count == rule.conditions.count)
        #expect(result?.actions.count == rule.actions.count)
    }

    @Test("CachedMetrics round-trips current metrics")
    func metricsRoundTrip() {
        let metrics: CurrentMetricsResponse = TestData.decode(TestData.currentMetricsJSON)
        let cached = CachedMetrics.from(current: metrics)
        let result = cached.toCurrentMetrics()

        #expect(result != nil)
        #expect(result?.system.cpuUsagePercent == metrics.system.cpuUsagePercent)
        #expect(result?.app.requestCount == metrics.app.requestCount)
    }
}
