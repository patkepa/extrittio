// Extrittio/Data/Cache/CacheManager.swift
import Foundation
import SwiftData

@ModelActor
actor CacheManager: CacheMetadataProvider {

    // MARK: - Device

    func cacheDevice(_ device: Device) {
        let id = device.id
        let descriptor = FetchDescriptor<CachedDevice>(predicate: #Predicate { $0.id == id })
        if let existing = try? modelContext.fetch(descriptor).first {
            existing.name = device.name
            existing.deviceTypeId = device.deviceTypeId
            existing.deviceTypeName = device.deviceTypeName
            existing.fleetId = device.fleetId
            existing.fleetName = device.fleetName
            existing.status = device.status
            existing.firmware = device.firmware
            existing.lastSeen = device.lastSeen
            existing.lastSeenAt = device.lastSeenAt
            existing.uptime = device.uptime
            existing.uptimeSeconds = device.uptimeSeconds
            existing.latestLatitude = device.latestLatitude
            existing.latestLongitude = device.latestLongitude
            existing.cachedAt = Date()
        } else {
            modelContext.insert(CachedDevice.from(device))
        }
        upsertEntry(for: .device(id: device.id), data: device)
        try? modelContext.save()
    }

    func cacheDevices(_ devices: [Device]) {
        for device in devices {
            let id = device.id
            let descriptor = FetchDescriptor<CachedDevice>(predicate: #Predicate { $0.id == id })
            if let existing = try? modelContext.fetch(descriptor).first {
                existing.name = device.name
                existing.deviceTypeId = device.deviceTypeId
                existing.deviceTypeName = device.deviceTypeName
                existing.fleetId = device.fleetId
                existing.fleetName = device.fleetName
                existing.status = device.status
                existing.firmware = device.firmware
                existing.lastSeen = device.lastSeen
                existing.lastSeenAt = device.lastSeenAt
                existing.uptime = device.uptime
                existing.uptimeSeconds = device.uptimeSeconds
                existing.latestLatitude = device.latestLatitude
                existing.latestLongitude = device.latestLongitude
                existing.cachedAt = Date()
            } else {
                modelContext.insert(CachedDevice.from(device))
            }
            upsertEntry(for: .device(id: device.id), data: device)
        }
        upsertEntry(for: .devices, data: devices)
        try? modelContext.save()
    }

    func getDevice(id: String) -> Device? {
        let descriptor = FetchDescriptor<CachedDevice>(predicate: #Predicate { $0.id == id })
        return try? modelContext.fetch(descriptor).first?.toDomain()
    }

    func getDevices(status: String? = nil, search: String? = nil, fleetId: Int? = nil) -> [Device]? {
        let descriptor = FetchDescriptor<CachedDevice>()
        guard let all = try? modelContext.fetch(descriptor), !all.isEmpty else { return nil }
        var filtered = all
        if let status {
            filtered = filtered.filter { $0.status == status }
        }
        if let search, !search.isEmpty {
            let lowered = search.lowercased()
            filtered = filtered.filter { $0.name.lowercased().contains(lowered) || $0.id.lowercased().contains(lowered) }
        }
        if let fleetId {
            filtered = filtered.filter { $0.fleetId == fleetId }
        }
        return filtered.isEmpty ? nil : filtered.map { $0.toDomain() }
    }

    func removeDevice(id: String) {
        let descriptor = FetchDescriptor<CachedDevice>(predicate: #Predicate { $0.id == id })
        if let existing = try? modelContext.fetch(descriptor).first {
            modelContext.delete(existing)
        }
        removeEntry(for: .device(id: id))
        try? modelContext.save()
    }

    // MARK: - Telemetry

    func cacheTelemetry(_ records: [TelemetryRecord], deviceId: String) {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedTelemetryRecord>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for record in records {
            modelContext.insert(CachedTelemetryRecord.from(record))
        }
        upsertEntry(for: .telemetry(deviceId: deviceId), data: records)
        try? modelContext.save()
    }

    func getTelemetry(deviceId: String) -> [TelemetryRecord]? {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedTelemetryRecord>(predicate: #Predicate { $0.deviceId == devId })
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        return results.map { $0.toDomain() }
    }

    // MARK: - Shadow

    func cacheShadow(_ shadow: DeviceShadow) {
        let devId = shadow.deviceId
        let descriptor = FetchDescriptor<CachedDeviceShadow>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor).first {
            let encoder = JSONEncoder()
            existing.desiredData = (try? encoder.encode(shadow.desired)) ?? Data()
            existing.reportedData = (try? encoder.encode(shadow.reported)) ?? Data()
            existing.deltaData = (try? encoder.encode(shadow.delta)) ?? Data()
            existing.version = shadow.version
            existing.updatedAt = shadow.updatedAt
            existing.cachedAt = Date()
        } else {
            modelContext.insert(CachedDeviceShadow.from(shadow))
        }
        upsertEntry(for: .shadow(deviceId: shadow.deviceId), data: shadow)
        try? modelContext.save()
    }

    func getShadow(deviceId: String) -> DeviceShadow? {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedDeviceShadow>(predicate: #Predicate { $0.deviceId == devId })
        return try? modelContext.fetch(descriptor).first?.toDomain()
    }

    func removeShadow(deviceId: String) {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedDeviceShadow>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor).first {
            modelContext.delete(existing)
        }
        removeEntry(for: .shadow(deviceId: deviceId))
        try? modelContext.save()
    }

    // MARK: - Commands

    func cacheCommands(_ commands: [CommandRecord], deviceId: String) {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedCommandRecord>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for cmd in commands {
            modelContext.insert(CachedCommandRecord.from(cmd))
        }
        upsertEntry(for: .commands(deviceId: deviceId), data: commands)
        try? modelContext.save()
    }

    func getCommands(deviceId: String) -> [CommandRecord]? {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedCommandRecord>(predicate: #Predicate { $0.deviceId == devId })
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        return results.map { $0.toDomain() }
    }

    // MARK: - Config

    func cacheConfig(_ config: [String: AnyCodableValue], deviceId: String) {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedDeviceConfig>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor).first {
            let encoder = JSONEncoder()
            existing.configData = (try? encoder.encode(config)) ?? Data()
            existing.cachedAt = Date()
        } else {
            modelContext.insert(CachedDeviceConfig.from(config, deviceId: deviceId))
        }
        upsertEntry(for: .config(deviceId: deviceId), data: config)
        try? modelContext.save()
    }

    func getConfig(deviceId: String) -> [String: AnyCodableValue]? {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedDeviceConfig>(predicate: #Predicate { $0.deviceId == devId })
        guard let result = try? modelContext.fetch(descriptor).first else { return nil }
        let domain = result.toDomain()
        return domain.isEmpty ? nil : domain
    }

    // MARK: - Logs

    func cacheLogs(_ logs: [DeviceLog], deviceId: String) {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedDeviceLog>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for log in logs {
            modelContext.insert(CachedDeviceLog.from(log))
        }
        upsertEntry(for: .logs(deviceId: deviceId), data: logs)
        try? modelContext.save()
    }

    func getLogs(deviceId: String) -> [DeviceLog]? {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedDeviceLog>(predicate: #Predicate { $0.deviceId == devId })
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        return results.map { $0.toDomain() }
    }

    // MARK: - Alerts

    func cacheAlerts(_ alerts: [Alert], deviceId: String) {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedAlert>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for alert in alerts {
            modelContext.insert(CachedAlert.from(alert))
        }
        upsertEntry(for: .alerts(deviceId: deviceId), data: alerts)
        try? modelContext.save()
    }

    func getAlerts(deviceId: String) -> [Alert]? {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedAlert>(predicate: #Predicate { $0.deviceId == devId })
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        return results.map { $0.toDomain() }
    }

    func removeAlert(id: String) {
        let alertId = id
        let descriptor = FetchDescriptor<CachedAlert>(predicate: #Predicate { $0.id == alertId })
        if let existing = try? modelContext.fetch(descriptor).first {
            modelContext.delete(existing)
        }
        try? modelContext.save()
    }

    // MARK: - Dashboard

    func cacheDashboardStats(_ stats: DashboardStats) {
        let key = "dashboard_stats"
        let descriptor = FetchDescriptor<CachedDashboardStats>(predicate: #Predicate { $0.key == key })
        if let existing = try? modelContext.fetch(descriptor).first {
            existing.totalDevices = stats.totalDevices
            existing.activeDevices = stats.activeDevices
            existing.offlineDevices = stats.offlineDevices
            existing.totalMessages = stats.totalMessages
            existing.cachedAt = Date()
        } else {
            modelContext.insert(CachedDashboardStats.from(stats))
        }
        upsertEntry(for: .dashboardStats, data: stats)
        try? modelContext.save()
    }

    func getDashboardStats() -> DashboardStats? {
        let key = "dashboard_stats"
        let descriptor = FetchDescriptor<CachedDashboardStats>(predicate: #Predicate { $0.key == key })
        return try? modelContext.fetch(descriptor).first?.toDomain()
    }

    func cacheAlertSummary(_ summary: AlertSummary) {
        let key = "alert_summary"
        let descriptor = FetchDescriptor<CachedAlertSummary>(predicate: #Predicate { $0.key == key })
        if let existing = try? modelContext.fetch(descriptor).first {
            existing.activeInfo = summary.active.info
            existing.activeWarning = summary.active.warning
            existing.activeCritical = summary.active.critical
            existing.acknowledgedInfo = summary.acknowledged.info
            existing.acknowledgedWarning = summary.acknowledged.warning
            existing.acknowledgedCritical = summary.acknowledged.critical
            existing.totalActive = summary.totalActive
            existing.cachedAt = Date()
        } else {
            modelContext.insert(CachedAlertSummary.from(summary))
        }
        upsertEntry(for: .alertSummary, data: summary)
        try? modelContext.save()
    }

    func getAlertSummary() -> AlertSummary? {
        let key = "alert_summary"
        let descriptor = FetchDescriptor<CachedAlertSummary>(predicate: #Predicate { $0.key == key })
        return try? modelContext.fetch(descriptor).first?.toDomain()
    }

    // MARK: - OTA

    func cacheOtaDeployments(_ deployments: [OtaDeployment], deviceId: String) {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedOtaDeployment>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for dep in deployments {
            modelContext.insert(CachedOtaDeployment.from(dep))
        }
        upsertEntry(for: .otaDeployments(deviceId: deviceId), data: deployments)
        try? modelContext.save()
    }

    func getOtaDeployments(deviceId: String) -> [OtaDeployment]? {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedOtaDeployment>(predicate: #Predicate { $0.deviceId == devId })
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        return results.map { $0.toDomain() }
    }

    // MARK: - Firmware

    func cacheFirmware(_ firmware: [FirmwareUpdate]) {
        let descriptor = FetchDescriptor<CachedFirmwareUpdate>()
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for fw in firmware {
            modelContext.insert(CachedFirmwareUpdate.from(fw))
        }
        upsertEntry(for: .firmware, data: firmware)
        try? modelContext.save()
    }

    func getFirmware() -> [FirmwareUpdate]? {
        let descriptor = FetchDescriptor<CachedFirmwareUpdate>()
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        return results.map { $0.toDomain() }
    }

    func removeFirmware(id: Int) {
        let fwId = id
        let descriptor = FetchDescriptor<CachedFirmwareUpdate>(predicate: #Predicate { $0.id == fwId })
        if let existing = try? modelContext.fetch(descriptor).first {
            modelContext.delete(existing)
        }
        try? modelContext.save()
    }

    // MARK: - Fleets

    func cacheFleets(_ fleets: [Fleet]) {
        let descriptor = FetchDescriptor<CachedFleet>()
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for fleet in fleets {
            modelContext.insert(CachedFleet.from(fleet))
        }
        upsertEntry(for: .fleets, data: fleets)
        try? modelContext.save()
    }

    func getFleets() -> [Fleet]? {
        let descriptor = FetchDescriptor<CachedFleet>()
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        return results.map { $0.toDomain() }
    }

    // MARK: - Device Types

    func cacheDeviceTypes(_ types: [DeviceType]) {
        let descriptor = FetchDescriptor<CachedDeviceType>()
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for dt in types {
            modelContext.insert(CachedDeviceType.from(dt))
        }
        upsertEntry(for: .deviceTypes, data: types)
        try? modelContext.save()
    }

    func getDeviceTypes() -> [DeviceType]? {
        let descriptor = FetchDescriptor<CachedDeviceType>()
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        return results.map { $0.toDomain() }
    }

    // MARK: - Location

    func cacheLocation(_ location: DeviceLocation, deviceId: String) {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedDeviceLocation>(predicate: #Predicate { $0.deviceId == devId })
        if let existing = try? modelContext.fetch(descriptor).first {
            existing.latitude = location.latitude
            existing.longitude = location.longitude
            existing.speed = location.speed
            existing.altitude = location.altitude
            existing.heading = location.heading
            existing.timestamp = location.timestamp
            existing.cachedAt = Date()
        } else {
            modelContext.insert(CachedDeviceLocation.from(location, deviceId: deviceId))
        }
        upsertEntry(for: .location(deviceId: deviceId), data: location)
        try? modelContext.save()
    }

    func getLocation(deviceId: String) -> DeviceLocation? {
        let devId = deviceId
        let descriptor = FetchDescriptor<CachedDeviceLocation>(predicate: #Predicate { $0.deviceId == devId })
        return try? modelContext.fetch(descriptor).first?.toDomain()
    }

    // MARK: - Metrics

    func cacheCurrentMetrics(_ metrics: CurrentMetricsResponse) {
        let key = "current"
        let descriptor = FetchDescriptor<CachedMetrics>(predicate: #Predicate { $0.key == key })
        if let existing = try? modelContext.fetch(descriptor).first {
            let encoder = JSONEncoder()
            existing.jsonData = (try? encoder.encode(metrics)) ?? Data()
            existing.cachedAt = Date()
        } else {
            modelContext.insert(CachedMetrics.from(current: metrics))
        }
        upsertEntry(for: .metrics, data: metrics)
        try? modelContext.save()
    }

    func getCurrentMetrics() -> CurrentMetricsResponse? {
        let key = "current"
        let descriptor = FetchDescriptor<CachedMetrics>(predicate: #Predicate { $0.key == key })
        return try? modelContext.fetch(descriptor).first?.toCurrentMetrics()
    }

    func cacheMetricsHistory(_ history: MetricsHistoryResponse) {
        let key = "history"
        let descriptor = FetchDescriptor<CachedMetrics>(predicate: #Predicate { $0.key == key })
        if let existing = try? modelContext.fetch(descriptor).first {
            let encoder = JSONEncoder()
            existing.jsonData = (try? encoder.encode(history)) ?? Data()
            existing.cachedAt = Date()
        } else {
            modelContext.insert(CachedMetrics.from(history: history))
        }
        upsertEntry(for: .metricsHistory, data: history)
        try? modelContext.save()
    }

    func getMetricsHistory() -> MetricsHistoryResponse? {
        let key = "history"
        let descriptor = FetchDescriptor<CachedMetrics>(predicate: #Predicate { $0.key == key })
        return try? modelContext.fetch(descriptor).first?.toMetricsHistory()
    }

    // MARK: - Rules

    func cacheRules(_ rules: [Rule]) {
        let descriptor = FetchDescriptor<CachedRule>()
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for rule in rules {
            modelContext.insert(CachedRule.from(rule))
        }
        upsertEntry(for: .rules, data: rules)
        try? modelContext.save()
    }

    func getRules() -> [Rule]? {
        let descriptor = FetchDescriptor<CachedRule>()
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        let rules = results.compactMap { $0.toDomain() }
        return rules.isEmpty ? nil : rules
    }

    func removeRule(id: String) {
        let ruleId = id
        let descriptor = FetchDescriptor<CachedRule>(predicate: #Predicate { $0.id == ruleId })
        if let existing = try? modelContext.fetch(descriptor).first {
            modelContext.delete(existing)
        }
        try? modelContext.save()
    }

    // MARK: - Zones

    func cacheZones(_ zones: [Zone]) {
        let descriptor = FetchDescriptor<CachedZone>()
        if let existing = try? modelContext.fetch(descriptor) {
            for record in existing { modelContext.delete(record) }
        }
        for zone in zones {
            modelContext.insert(CachedZone.from(zone))
        }
        upsertEntry(for: .zones, data: zones)
        try? modelContext.save()
    }

    func getZones() -> [Zone]? {
        let descriptor = FetchDescriptor<CachedZone>()
        guard let results = try? modelContext.fetch(descriptor), !results.isEmpty else { return nil }
        let zones = results.compactMap { $0.toDomain() }
        return zones.isEmpty ? nil : zones
    }

    // MARK: - CacheMetadataProvider

    func lastUpdated(for key: CacheKey) -> Date? {
        let keyString = key.stringValue
        let descriptor = FetchDescriptor<CacheEntry>(predicate: #Predicate { $0.key == keyString })
        return try? modelContext.fetch(descriptor).first?.lastUpdated
    }

    func totalCacheSize() -> Int64 {
        let descriptor = FetchDescriptor<CacheEntry>()
        guard let entries = try? modelContext.fetch(descriptor) else { return 0 }
        return entries.reduce(0) { $0 + Int64($1.sizeBytes) }
    }

    // MARK: - Eviction

    func evictExpired(maxAge: TimeInterval = 86_400) {
        let cutoff = Date().addingTimeInterval(-maxAge)
        deleteExpired(CachedDevice.self, cutoff: cutoff)
        deleteExpired(CachedDashboardStats.self, cutoff: cutoff)
        deleteExpired(CachedAlertSummary.self, cutoff: cutoff)
        deleteExpired(CachedTelemetryRecord.self, cutoff: cutoff)
        deleteExpired(CachedDeviceShadow.self, cutoff: cutoff)
        deleteExpired(CachedCommandRecord.self, cutoff: cutoff)
        deleteExpired(CachedDeviceConfig.self, cutoff: cutoff)
        deleteExpired(CachedDeviceLog.self, cutoff: cutoff)
        deleteExpired(CachedAlert.self, cutoff: cutoff)
        deleteExpired(CachedOtaDeployment.self, cutoff: cutoff)
        deleteExpired(CachedFirmwareUpdate.self, cutoff: cutoff)
        deleteExpired(CachedFleet.self, cutoff: cutoff)
        deleteExpired(CachedDeviceType.self, cutoff: cutoff)
        deleteExpired(CachedDeviceLocation.self, cutoff: cutoff)
        deleteExpired(CachedMetrics.self, cutoff: cutoff)
        deleteExpired(CachedRule.self, cutoff: cutoff)
        deleteExpired(CachedZone.self, cutoff: cutoff)

        let descriptor = FetchDescriptor<CacheEntry>(predicate: #Predicate { $0.lastUpdated < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for entry in expired { modelContext.delete(entry) }
        }
        try? modelContext.save()
    }

    func evictIfOverSize(maxBytes: Int64) {
        let descriptor = FetchDescriptor<CacheEntry>(sortBy: [SortDescriptor(\.lastUpdated, order: .forward)])
        guard let entries = try? modelContext.fetch(descriptor) else { return }
        var total: Int64 = entries.reduce(0) { $0 + Int64($1.sizeBytes) }
        for entry in entries {
            if total <= maxBytes { break }
            total -= Int64(entry.sizeBytes)
            deleteModelRecords(forKey: entry.key)
            modelContext.delete(entry)
        }
        try? modelContext.save()
    }

    func clearAll() {
        deleteAll(CachedDevice.self)
        deleteAll(CachedDashboardStats.self)
        deleteAll(CachedAlertSummary.self)
        deleteAll(CachedTelemetryRecord.self)
        deleteAll(CachedDeviceShadow.self)
        deleteAll(CachedCommandRecord.self)
        deleteAll(CachedDeviceConfig.self)
        deleteAll(CachedDeviceLog.self)
        deleteAll(CachedAlert.self)
        deleteAll(CachedOtaDeployment.self)
        deleteAll(CachedFirmwareUpdate.self)
        deleteAll(CachedFleet.self)
        deleteAll(CachedDeviceType.self)
        deleteAll(CachedDeviceLocation.self)
        deleteAll(CachedMetrics.self)
        deleteAll(CachedRule.self)
        deleteAll(CachedZone.self)
        deleteAll(CacheEntry.self)
        try? modelContext.save()
    }

    // MARK: - Private Helpers

    private func upsertEntry<T: Encodable>(for key: CacheKey, data: T) {
        let encoder = JSONEncoder()
        let size = (try? encoder.encode(data))?.count ?? 100
        upsertEntry(for: key, estimatedSize: size)
    }

    private func upsertEntry(for key: CacheKey, estimatedSize: Int) {
        let keyString = key.stringValue
        let descriptor = FetchDescriptor<CacheEntry>(predicate: #Predicate { $0.key == keyString })
        if let existing = try? modelContext.fetch(descriptor).first {
            existing.lastUpdated = Date()
            existing.sizeBytes = estimatedSize
        } else {
            modelContext.insert(CacheEntry(key: keyString, lastUpdated: Date(), sizeBytes: estimatedSize))
        }
        try? modelContext.save()
    }

    private func removeEntry(for key: CacheKey) {
        let keyString = key.stringValue
        let descriptor = FetchDescriptor<CacheEntry>(predicate: #Predicate { $0.key == keyString })
        if let existing = try? modelContext.fetch(descriptor).first {
            modelContext.delete(existing)
            try? modelContext.save()
        }
    }

    private func deleteAll<T: PersistentModel>(_ type: T.Type) {
        let descriptor = FetchDescriptor<T>()
        if let all = try? modelContext.fetch(descriptor) {
            for item in all { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedDevice.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedDevice>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedDashboardStats.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedDashboardStats>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedAlertSummary.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedAlertSummary>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedTelemetryRecord.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedTelemetryRecord>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedDeviceShadow.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedDeviceShadow>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedCommandRecord.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedCommandRecord>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedDeviceConfig.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedDeviceConfig>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedDeviceLog.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedDeviceLog>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedAlert.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedAlert>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedOtaDeployment.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedOtaDeployment>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedFirmwareUpdate.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedFirmwareUpdate>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedFleet.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedFleet>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedDeviceType.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedDeviceType>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedDeviceLocation.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedDeviceLocation>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedMetrics.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedMetrics>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedRule.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedRule>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteExpired(_ type: CachedZone.Type, cutoff: Date) {
        let descriptor = FetchDescriptor<CachedZone>(predicate: #Predicate { $0.cachedAt < cutoff })
        if let expired = try? modelContext.fetch(descriptor) {
            for item in expired { modelContext.delete(item) }
        }
    }

    private func deleteModelRecords(forKey key: String) {
        if key.hasPrefix("device:") {
            let id = String(key.dropFirst("device:".count))
            let descriptor = FetchDescriptor<CachedDevice>(predicate: #Predicate { $0.id == id })
            if let item = try? modelContext.fetch(descriptor).first { modelContext.delete(item) }
        } else if key == "devices" {
            deleteAll(CachedDevice.self)
        } else if key == "dashboard_stats" {
            deleteAll(CachedDashboardStats.self)
        } else if key == "alert_summary" {
            deleteAll(CachedAlertSummary.self)
        } else if key.hasPrefix("telemetry:") {
            let devId = String(key.dropFirst("telemetry:".count))
            let descriptor = FetchDescriptor<CachedTelemetryRecord>(predicate: #Predicate { $0.deviceId == devId })
            if let items = try? modelContext.fetch(descriptor) {
                for item in items { modelContext.delete(item) }
            }
        } else if key.hasPrefix("shadow:") {
            let devId = String(key.dropFirst("shadow:".count))
            let descriptor = FetchDescriptor<CachedDeviceShadow>(predicate: #Predicate { $0.deviceId == devId })
            if let item = try? modelContext.fetch(descriptor).first { modelContext.delete(item) }
        } else if key.hasPrefix("commands:") {
            let devId = String(key.dropFirst("commands:".count))
            let descriptor = FetchDescriptor<CachedCommandRecord>(predicate: #Predicate { $0.deviceId == devId })
            if let items = try? modelContext.fetch(descriptor) {
                for item in items { modelContext.delete(item) }
            }
        } else if key.hasPrefix("config:") {
            let devId = String(key.dropFirst("config:".count))
            let descriptor = FetchDescriptor<CachedDeviceConfig>(predicate: #Predicate { $0.deviceId == devId })
            if let item = try? modelContext.fetch(descriptor).first { modelContext.delete(item) }
        } else if key.hasPrefix("logs:") {
            let devId = String(key.dropFirst("logs:".count))
            let descriptor = FetchDescriptor<CachedDeviceLog>(predicate: #Predicate { $0.deviceId == devId })
            if let items = try? modelContext.fetch(descriptor) {
                for item in items { modelContext.delete(item) }
            }
        } else if key.hasPrefix("alerts:") {
            let devId = String(key.dropFirst("alerts:".count))
            let descriptor = FetchDescriptor<CachedAlert>(predicate: #Predicate { $0.deviceId == devId })
            if let items = try? modelContext.fetch(descriptor) {
                for item in items { modelContext.delete(item) }
            }
        } else if key.hasPrefix("ota_deployments:") {
            let devId = String(key.dropFirst("ota_deployments:".count))
            let descriptor = FetchDescriptor<CachedOtaDeployment>(predicate: #Predicate { $0.deviceId == devId })
            if let items = try? modelContext.fetch(descriptor) {
                for item in items { modelContext.delete(item) }
            }
        } else if key == "firmware" {
            deleteAll(CachedFirmwareUpdate.self)
        } else if key == "fleets" {
            deleteAll(CachedFleet.self)
        } else if key == "device_types" {
            deleteAll(CachedDeviceType.self)
        } else if key.hasPrefix("location:") {
            let devId = String(key.dropFirst("location:".count))
            let descriptor = FetchDescriptor<CachedDeviceLocation>(predicate: #Predicate { $0.deviceId == devId })
            if let item = try? modelContext.fetch(descriptor).first { modelContext.delete(item) }
        } else if key == "metrics" {
            let k = "current"
            let descriptor = FetchDescriptor<CachedMetrics>(predicate: #Predicate { $0.key == k })
            if let item = try? modelContext.fetch(descriptor).first { modelContext.delete(item) }
        } else if key == "metrics_history" {
            let k = "history"
            let descriptor = FetchDescriptor<CachedMetrics>(predicate: #Predicate { $0.key == k })
            if let item = try? modelContext.fetch(descriptor).first { modelContext.delete(item) }
        } else if key == "rules" {
            deleteAll(CachedRule.self)
        } else if key == "zones" {
            deleteAll(CachedZone.self)
        }
    }
}
