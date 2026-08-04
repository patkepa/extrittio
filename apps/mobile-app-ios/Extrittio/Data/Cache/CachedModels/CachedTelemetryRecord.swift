// Extrittio/Data/Cache/CachedModels/CachedTelemetryRecord.swift
import Foundation
import SwiftData

@Model
final class CachedTelemetryRecord {
    @Attribute(.unique) var id: Int
    var deviceId: String
    var temperature: Double?
    var humidity: Double?
    var batteryLevel: Double?
    var customJson: String?
    var latitude: Double?
    var longitude: Double?
    var speed: Double?
    var altitude: Double?
    var heading: Double?
    var receivedAt: String
    var cachedAt: Date

    init(id: Int, deviceId: String, temperature: Double?, humidity: Double?,
         batteryLevel: Double?, customJson: String?, latitude: Double?, longitude: Double?,
         speed: Double?, altitude: Double?, heading: Double?, receivedAt: String,
         cachedAt: Date = Date()) {
        self.id = id; self.deviceId = deviceId; self.temperature = temperature
        self.humidity = humidity; self.batteryLevel = batteryLevel; self.customJson = customJson
        self.latitude = latitude; self.longitude = longitude; self.speed = speed
        self.altitude = altitude; self.heading = heading; self.receivedAt = receivedAt
        self.cachedAt = cachedAt
    }

    func toDomain() -> TelemetryRecord {
        TelemetryRecord(id: id, deviceId: deviceId, temperature: temperature,
                        humidity: humidity, batteryLevel: batteryLevel, customJson: customJson,
                        latitude: latitude, longitude: longitude, speed: speed,
                        altitude: altitude, heading: heading, receivedAt: receivedAt)
    }

    static func from(_ record: TelemetryRecord) -> CachedTelemetryRecord {
        CachedTelemetryRecord(id: record.id, deviceId: record.deviceId,
                              temperature: record.temperature, humidity: record.humidity,
                              batteryLevel: record.batteryLevel, customJson: record.customJson,
                              latitude: record.latitude, longitude: record.longitude,
                              speed: record.speed, altitude: record.altitude,
                              heading: record.heading, receivedAt: record.receivedAt)
    }
}
