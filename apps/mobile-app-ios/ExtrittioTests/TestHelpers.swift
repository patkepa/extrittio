import Foundation
@testable import Extrittio

enum TestData {
    static let dashboardStatsJSON = """
    {"total_devices":10,"active_devices":7,"offline_devices":3,"total_messages":1500}
    """

    static let alertSummaryJSON = """
    {"active":{"info":2,"warning":1,"critical":0},"acknowledged":{"info":0,"warning":0,"critical":0},"total_active":3}
    """

    static let deviceJSON = """
    {"id":"abc-123","name":"Sensor-01","device_type_id":1,"device_type_name":"Temperature Sensor","fleet_id":1,"fleet_name":"Floor 1","status":"online","firmware":"1.0.0","last_seen":"2025-01-15T10:30:00","last_seen_at":"2025-01-15T10:30:00","uptime":"2d 3h","uptime_seconds":183600}
    """

    static let deviceListJSON = """
    {"data":[\(deviceJSON)],"total":1,"limit":30,"offset":0}
    """

    static let telemetryJSON = """
    [{"id":1,"device_id":"abc-123","temperature":22.5,"humidity":45.0,"battery_level":87.0,"custom_json":null,"received_at":"2025-01-15T10:30:00"}]
    """

    static let shadowJSON = """
    {"device_id":"abc-123","desired":{"led":"on"},"reported":{"led":"off"},"delta":{"led":"on"},"version":3,"updated_at":"2025-01-15T10:30:00"}
    """

    static let commandJSON = """
    {"id":"cmd-1","device_id":"abc-123","command":"restart","params":null,"status":"success","response_payload":null,"created_at":"2025-01-15T10:30:00","updated_at":"2025-01-15T10:31:00"}
    """

    static let logJSON = """
    [{"id":1,"device_id":"abc-123","level":"INFO","message":"Device started","created_at":"2025-01-15T10:30:00"}]
    """

    static let alertJSON = """
    {"id":"alert-1","rule_id":"rule-1","device_id":"abc-123","severity":"warning","status":"active","message":"Temperature high","triggered_value":"35.2","resolved_at":null,"acknowledged_at":null,"created_at":"2025-01-15T10:30:00"}
    """

    static let ruleJSON = """
    {"id":"rule-1","name":"High Temp Alert","description":"Alert when temp exceeds threshold","enabled":true,"trigger_type":"telemetry","target_type":"global","target_id":null,"cooldown_seconds":300,"conditions":[{"id":"cond-1","field":"temperature","operator":"gt","value":"30"}],"actions":[{"id":"act-1","action_type":"create_alert","config":{"severity":"warning"}}],"created_at":"2025-01-15T10:00:00","updated_at":"2025-01-15T10:00:00"}
    """

    static let firmwareJSON = """
    {"id":1,"device_type_id":1,"device_type_name":"Temperature Sensor","version":"2.0.0","url":"https://example.com/fw.bin","sha256":null,"description":"Bug fixes","created_at":"2025-01-15T10:00:00","has_blob":false,"file_size":null,"filename":null,"source":"manual"}
    """

    static let nestedShadowJSON = """
    {"device_id":"abc-123","desired":{"config":{"interval":30,"enabled":true},"tags":["indoor","floor1"]},"reported":{"config":{"interval":60,"enabled":false}},"delta":{"config":{"interval":30,"enabled":true}},"version":5,"updated_at":"2025-01-15T10:30:00"}
    """

    static let currentMetricsJSON = """
    {
        "system": {
            "cpu_usage_percent": 23.5,
            "memory_used_bytes": 5263728640,
            "memory_total_bytes": 8589934592,
            "disk_used_bytes": 123480309760,
            "disk_total_bytes": 274877906944,
            "network_rx_bytes_delta": 1258291,
            "network_tx_bytes_delta": 348160,
            "load_avg_1m": 0.82,
            "load_avg_5m": 1.21,
            "load_avg_15m": 0.95,
            "recorded_at": "2026-03-17T10:30:00"
        },
        "app": {
            "request_count": 142,
            "error_count": 0,
            "avg_latency_ms": 12.3,
            "p95_latency_ms": 28.1,
            "db_pool_active": 3,
            "db_pool_idle": 7,
            "zenoh_messages_in": 48,
            "zenoh_messages_out": 12,
            "recorded_at": "2026-03-17T10:30:00"
        }
    }
    """

    static let currentMetricsNoLoadAvgJSON = """
    {
        "system": {
            "cpu_usage_percent": 45.0,
            "memory_used_bytes": 4294967296,
            "memory_total_bytes": 8589934592,
            "disk_used_bytes": 107374182400,
            "disk_total_bytes": 274877906944,
            "network_rx_bytes_delta": 500000,
            "network_tx_bytes_delta": 100000,
            "recorded_at": "2026-03-17T10:31:00"
        },
        "app": {
            "request_count": 98,
            "error_count": 2,
            "avg_latency_ms": 18.7,
            "p95_latency_ms": 45.2,
            "db_pool_active": 5,
            "db_pool_idle": 5,
            "zenoh_messages_in": 30,
            "zenoh_messages_out": 8,
            "recorded_at": "2026-03-17T10:31:00"
        }
    }
    """

    static func decode<T: Decodable>(_ json: String) -> T {
        try! JSONDecoder().decode(T.self, from: json.data(using: .utf8)!)
    }
}
