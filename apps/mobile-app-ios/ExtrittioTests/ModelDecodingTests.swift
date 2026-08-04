import Testing
@testable import Extrittio

private func makeDevice(firmware: String) -> Device {
    Device(
        id: "abc-123",
        name: "Sensor-01",
        deviceTypeId: 1,
        deviceTypeName: "Temperature Sensor",
        fleetId: 1,
        fleetName: "Floor 1",
        status: "online",
        firmware: firmware,
        lastSeen: nil,
        lastSeenAt: nil,
        uptime: nil,
        uptimeSeconds: 0,
        latestLatitude: nil,
        latestLongitude: nil
    )
}

@Test func decodeDashboardStats() {
    let stats: DashboardStats = TestData.decode(TestData.dashboardStatsJSON)
    #expect(stats.totalDevices == 10)
    #expect(stats.activeDevices == 7)
    #expect(stats.offlineDevices == 3)
    #expect(stats.totalMessages == 1500)
}

@Test func decodeAlertSummary() {
    let summary: AlertSummary = TestData.decode(TestData.alertSummaryJSON)
    #expect(summary.totalActive == 3)
    #expect(summary.active.info == 2)
    #expect(summary.active.warning == 1)
}

@Test func decodeDevice() {
    let device: Device = TestData.decode(TestData.deviceJSON)
    #expect(device.id == "abc-123")
    #expect(device.name == "Sensor-01")
    #expect(device.isOnline)
    #expect(device.deviceTypeName == "Temperature Sensor")
    #expect(device.fleetName == "Floor 1")
    #expect(device.uptimeSeconds == 183600)
}

@Test func deviceFirmwareVersionLabel() {
    let known = makeDevice(firmware: "1.2.3")
    let prefixed = makeDevice(firmware: "v1.2.3")
    let unknown = makeDevice(firmware: "unknown")
    let empty = makeDevice(firmware: " ")

    #expect(known.firmwareVersionLabel == "v1.2.3")
    #expect(prefixed.firmwareVersionLabel == "v1.2.3")
    #expect(unknown.firmwareVersionLabel == "Unknown")
    #expect(empty.firmwareVersionLabel == "Unknown")
}

@Test func decodeDeviceList() {
    let response: PaginatedResponse<Device> = TestData.decode(TestData.deviceListJSON)
    #expect(response.data.count == 1)
    #expect(response.total == 1)
}

@Test func decodeTelemetry() {
    let records: [TelemetryRecord] = TestData.decode(TestData.telemetryJSON)
    #expect(records.count == 1)
    #expect(records[0].temperature == 22.5)
    #expect(records[0].humidity == 45.0)
    #expect(records[0].batteryLevel == 87.0)
}

@Test func decodeShadow() {
    let shadow: DeviceShadow = TestData.decode(TestData.shadowJSON)
    #expect(shadow.version == 3)
    #expect(shadow.desired["led"] == .string("on"))
    #expect(shadow.reported["led"] == .string("off"))
    #expect(shadow.delta["led"] == .string("on"))
}

@Test func decodeCommand() {
    let cmd: CommandRecord = TestData.decode(TestData.commandJSON)
    #expect(cmd.command == "restart")
    #expect(cmd.isSuccess)
    #expect(cmd.deviceId == "abc-123")
}

@Test func decodeDeviceLog() {
    let logs: [DeviceLog] = TestData.decode(TestData.logJSON)
    #expect(logs.count == 1)
    #expect(logs[0].level == "INFO")
    #expect(logs[0].message == "Device started")
}

@Test func decodeAlert() {
    let alert: Alert = TestData.decode(TestData.alertJSON)
    #expect(alert.severity == "warning")
    #expect(alert.isActive)
    #expect(alert.triggeredValue == "35.2")
}

@Test func decodeRule() {
    let rule: Rule = TestData.decode(TestData.ruleJSON)
    #expect(rule.id == "rule-1")
    #expect(rule.name == "High Temp Alert")
    #expect(rule.enabled)
    #expect(rule.triggerType == "telemetry")
    #expect(rule.targetType == "global")
    #expect(rule.cooldownSeconds == 300)
    #expect(rule.conditions.count == 1)
    #expect(rule.conditions[0].field == "temperature")
    #expect(rule.conditions[0].operator == "gt")
    #expect(rule.conditions[0].operatorLabel == ">")
    #expect(rule.actions.count == 1)
    #expect(rule.actions[0].actionType == "create_alert")
}

@Test func decodeFirmwareUpdate() {
    let fw: FirmwareUpdate = TestData.decode(TestData.firmwareJSON)
    #expect(fw.id == 1)
    #expect(fw.version == "2.0.0")
    #expect(fw.deviceTypeName == "Temperature Sensor")
    #expect(fw.source == "manual")
    #expect(fw.hasBlob == false)
}

@Test func anyCodableValueTypes() {
    let str = AnyCodableValue.string("hello")
    #expect(str.displayString == "hello")
    let num = AnyCodableValue.int(42)
    #expect(num.displayString == "42")
    let dbl = AnyCodableValue.double(3.14)
    #expect(dbl.displayString == "3.14")
    let boolean = AnyCodableValue.bool(true)
    #expect(boolean.displayString == "true")
    let null = AnyCodableValue.null
    #expect(null.displayString == "null")
}

@Test func decodeNestedShadow() {
    let shadow: DeviceShadow = TestData.decode(TestData.nestedShadowJSON)
    #expect(shadow.version == 5)
    if case .object(let config) = shadow.desired["config"] {
        #expect(config["interval"] == .int(30))
        #expect(config["enabled"] == .bool(true))
    } else {
        Issue.record("Expected .object for config")
    }
    if case .array(let tags) = shadow.desired["tags"] {
        #expect(tags.count == 2)
        #expect(tags[0] == .string("indoor"))
    } else {
        Issue.record("Expected .array for tags")
    }
}

@Test func anyCodableValueNestedTypes() {
    let obj = AnyCodableValue.object(["key": .string("val")])
    #expect(obj.displayString.contains("key"))
    let arr = AnyCodableValue.array([.int(1), .int(2)])
    #expect(arr.displayString.contains("1"))
}

@Test func anyCodableValueNestedEquality() {
    let a = AnyCodableValue.object(["x": .int(1)])
    let b = AnyCodableValue.object(["x": .int(1)])
    let c = AnyCodableValue.object(["x": .int(2)])
    #expect(a == b)
    #expect(a != c)
    let d = AnyCodableValue.array([.string("a")])
    let e = AnyCodableValue.array([.string("a")])
    #expect(d == e)
}

@Test func decodeCurrentMetrics() {
    let metrics: CurrentMetricsResponse = TestData.decode(TestData.currentMetricsJSON)
    #expect(metrics.system.cpuUsagePercent == 23.5)
    #expect(metrics.system.memoryUsedBytes == 5_263_728_640)
    #expect(metrics.system.memoryTotalBytes == 8_589_934_592)
    #expect(metrics.system.diskUsedBytes == 123_480_309_760)
    #expect(metrics.system.diskTotalBytes == 274_877_906_944)
    #expect(metrics.system.networkRxBytesDelta == 1_258_291)
    #expect(metrics.system.networkTxBytesDelta == 348_160)
    #expect(metrics.system.loadAvg1m == 0.82)
    #expect(metrics.system.loadAvg5m == 1.21)
    #expect(metrics.system.loadAvg15m == 0.95)
    #expect(metrics.system.recordedAt == "2026-03-17T10:30:00")
    #expect(metrics.app.requestCount == 142)
    #expect(metrics.app.errorCount == 0)
    #expect(metrics.app.avgLatencyMs == 12.3)
    #expect(metrics.app.p95LatencyMs == 28.1)
    #expect(metrics.app.dbPoolActive == 3)
    #expect(metrics.app.dbPoolIdle == 7)
    #expect(metrics.app.zenohMessagesIn == 48)
    #expect(metrics.app.zenohMessagesOut == 12)
}

@Test func decodeCurrentMetricsWithoutLoadAvg() {
    let metrics: CurrentMetricsResponse = TestData.decode(TestData.currentMetricsNoLoadAvgJSON)
    #expect(metrics.system.cpuUsagePercent == 45.0)
    #expect(metrics.system.loadAvg1m == nil)
    #expect(metrics.system.loadAvg5m == nil)
    #expect(metrics.system.loadAvg15m == nil)
    #expect(metrics.app.errorCount == 2)
}
