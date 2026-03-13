CREATE TABLE server_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    cpu_usage_percent REAL NOT NULL,
    memory_used_bytes BIGINT NOT NULL,
    memory_total_bytes BIGINT NOT NULL,
    disk_used_bytes BIGINT NOT NULL,
    disk_total_bytes BIGINT NOT NULL,
    network_rx_bytes_delta BIGINT NOT NULL,
    network_tx_bytes_delta BIGINT NOT NULL,
    load_avg_1m REAL NOT NULL,
    load_avg_5m REAL NOT NULL,
    load_avg_15m REAL NOT NULL,
    recorded_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_server_metrics_recorded_at ON server_metrics(recorded_at);

CREATE TABLE app_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    request_count INTEGER NOT NULL,
    error_count INTEGER NOT NULL,
    avg_latency_ms REAL NOT NULL,
    p95_latency_ms REAL NOT NULL,
    db_pool_active INTEGER NOT NULL,
    db_pool_idle INTEGER NOT NULL,
    zenoh_messages_in INTEGER NOT NULL,
    zenoh_messages_out INTEGER NOT NULL,
    recorded_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_app_metrics_recorded_at ON app_metrics(recorded_at);
