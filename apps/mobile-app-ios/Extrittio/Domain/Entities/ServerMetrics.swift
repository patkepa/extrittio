import Foundation

struct CurrentMetricsResponse: Codable, Sendable {
    let system: SystemMetrics
    let app: AppMetrics
}

struct MetricsHistoryResponse: Codable, Sendable {
    let system: [SystemMetrics]
    let app: [AppMetrics]
}

struct SystemMetrics: Codable, Sendable {
    let cpuUsagePercent: Double
    let memoryUsedBytes: Int64
    let memoryTotalBytes: Int64
    let diskUsedBytes: Int64
    let diskTotalBytes: Int64
    let networkRxBytesDelta: Int64
    let networkTxBytesDelta: Int64
    let loadAvg1m: Double?
    let loadAvg5m: Double?
    let loadAvg15m: Double?
    let recordedAt: String

    enum CodingKeys: String, CodingKey {
        case cpuUsagePercent = "cpu_usage_percent"
        case memoryUsedBytes = "memory_used_bytes"
        case memoryTotalBytes = "memory_total_bytes"
        case diskUsedBytes = "disk_used_bytes"
        case diskTotalBytes = "disk_total_bytes"
        case networkRxBytesDelta = "network_rx_bytes_delta"
        case networkTxBytesDelta = "network_tx_bytes_delta"
        case loadAvg1m = "load_avg_1m"
        case loadAvg5m = "load_avg_5m"
        case loadAvg15m = "load_avg_15m"
        case recordedAt = "recorded_at"
    }
}

struct AppMetrics: Codable, Sendable {
    let requestCount: Int
    let errorCount: Int
    let avgLatencyMs: Double
    let p95LatencyMs: Double
    let dbPoolActive: Int
    let dbPoolIdle: Int
    let zenohMessagesIn: Int
    let zenohMessagesOut: Int
    let recordedAt: String

    enum CodingKeys: String, CodingKey {
        case requestCount = "request_count"
        case errorCount = "error_count"
        case avgLatencyMs = "avg_latency_ms"
        case p95LatencyMs = "p95_latency_ms"
        case dbPoolActive = "db_pool_active"
        case dbPoolIdle = "db_pool_idle"
        case zenohMessagesIn = "zenoh_messages_in"
        case zenohMessagesOut = "zenoh_messages_out"
        case recordedAt = "recorded_at"
    }
}
