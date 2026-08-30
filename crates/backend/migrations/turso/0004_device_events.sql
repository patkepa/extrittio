CREATE TABLE device_events (
  id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL, contract_id TEXT NOT NULL, route_key TEXT NOT NULL,
  occurred_at INTEGER NOT NULL, received_at INTEGER NOT NULL,
  payload TEXT NOT NULL CHECK(json_valid(payload)), UNIQUE(tenant_id, device_id, id),
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE,
  FOREIGN KEY (tenant_id, contract_id) REFERENCES device_contracts(tenant_id, id)
);
CREATE TABLE device_metric_samples (
  event_id TEXT NOT NULL REFERENCES device_events(id) ON DELETE CASCADE,
  tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL, stream_key TEXT NOT NULL, field_path TEXT NOT NULL,
  value_type TEXT NOT NULL CHECK(value_type IN ('float64','int64','string','boolean','json')),
  value_double REAL, value_int INTEGER, value_text TEXT, value_bool INTEGER,
  value_json TEXT CHECK(value_json IS NULL OR json_valid(value_json)), occurred_at INTEGER NOT NULL,
  PRIMARY KEY(event_id, stream_key, field_path),
  FOREIGN KEY (tenant_id, device_id) REFERENCES devices(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_device_events_device_time
  ON device_events(tenant_id, device_id, occurred_at DESC);
CREATE INDEX idx_device_metric_samples_query
  ON device_metric_samples(tenant_id, device_id, stream_key, field_path, occurred_at DESC);
