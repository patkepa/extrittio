use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{Map, Value};
use std::collections::HashMap;
use tracing::warn;

const MAX_DECLARED_CONNECTIONS: usize = 256;

#[derive(Debug, Clone)]
pub struct ObservedNetworkHost {
    pub host_key: String,
    pub label: String,
    pub address: Option<String>,
    pub device_type: Option<String>,
    pub source: Option<String>,
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key)?.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_connection(value: &Value) -> Option<Value> {
    if !value.is_object() {
        return None;
    }

    let device_id = string_field(value, &["device_id", "target_device_id"]);
    let external_id = string_field(value, &["external_id", "id"]);
    let address = string_field(value, &["address", "ip", "mac"]);
    let label = string_field(value, &["label", "name", "hostname"])
        .or_else(|| device_id.clone())
        .or_else(|| external_id.clone())
        .or_else(|| address.clone())?;
    let connection_type = string_field(value, &["connection_type", "type", "kind"])
        .unwrap_or_else(|| "declared".to_string());

    let mut normalized = Map::new();
    normalized.insert(
        "id".to_string(),
        Value::String(
            external_id
                .clone()
                .or_else(|| device_id.clone())
                .or_else(|| address.clone())
                .unwrap_or_else(|| label.clone()),
        ),
    );
    normalized.insert("label".to_string(), Value::String(label));
    normalized.insert(
        "connection_type".to_string(),
        Value::String(connection_type),
    );

    if let Some(device_id) = device_id {
        normalized.insert("device_id".to_string(), Value::String(device_id));
    }
    if let Some(external_id) = external_id {
        normalized.insert("external_id".to_string(), Value::String(external_id));
    }
    if let Some(address) = address {
        normalized.insert("address".to_string(), Value::String(address));
    }
    if let Some(device_type) = string_field(value, &["device_type", "target_type"]) {
        normalized.insert("device_type".to_string(), Value::String(device_type));
    }
    if let Some(status) = string_field(value, &["status"]) {
        normalized.insert("status".to_string(), Value::String(status));
    }
    if let Some(source) = string_field(value, &["source"]) {
        normalized.insert("source".to_string(), Value::String(source));
    }

    Some(Value::Object(normalized))
}

fn normalize_connection_array(value: &Value) -> Option<Value> {
    let connections = value.as_array()?;
    let normalized: Vec<Value> = connections
        .iter()
        .take(MAX_DECLARED_CONNECTIONS)
        .filter_map(normalize_connection)
        .collect();
    Some(Value::Array(normalized))
}

pub fn network_analyzer_hosts(snapshot_json: &str) -> Option<Vec<ObservedNetworkHost>> {
    let snapshot: Value = serde_json::from_str(snapshot_json).ok()?;
    let hosts = snapshot.get("hosts")?.as_array()?;

    Some(
        hosts
            .iter()
            .take(MAX_DECLARED_CONNECTIONS)
            .filter_map(|host| {
                let ip = string_field(host, &["ip"]);
                let mac = string_field(host, &["mac"]);
                let hostname = string_field(host, &["hostname"]);
                let host_key = mac
                    .clone()
                    .filter(|m| m != "00:00:00:00:00:00")
                    .or_else(|| ip.clone())?;
                let label = hostname
                    .clone()
                    .or_else(|| ip.clone())
                    .unwrap_or_else(|| host_key.clone());

                Some(ObservedNetworkHost {
                    host_key,
                    label,
                    address: ip,
                    device_type: string_field(host, &["device_type", "classification"]),
                    source: string_field(host, &["source"]),
                })
            })
            .collect(),
    )
}

pub fn network_host_connection(
    host: &ObservedNetworkHost,
    status: &str,
    first_seen_at: Option<NaiveDateTime>,
    last_seen_at: Option<NaiveDateTime>,
) -> Value {
    let mut connection = Map::new();
    connection.insert(
        "id".to_string(),
        Value::String(format!("network-host:{}", host.host_key)),
    );
    connection.insert("label".to_string(), Value::String(host.label.clone()));
    connection.insert(
        "connection_type".to_string(),
        Value::String("network_host".to_string()),
    );
    connection.insert(
        "external_id".to_string(),
        Value::String(host.host_key.clone()),
    );
    if let Some(address) = &host.address {
        connection.insert("address".to_string(), Value::String(address.clone()));
    }
    if let Some(device_type) = &host.device_type {
        connection.insert(
            "device_type".to_string(),
            Value::String(device_type.clone()),
        );
    }
    connection.insert("status".to_string(), Value::String(status.to_string()));
    if let Some(source) = &host.source {
        connection.insert("source".to_string(), Value::String(source.clone()));
    }
    if let Some(first_seen_at) = first_seen_at {
        connection.insert(
            "first_seen_at".to_string(),
            Value::String(
                DateTime::<Utc>::from_naive_utc_and_offset(first_seen_at, Utc).to_rfc3339(),
            ),
        );
    }
    if let Some(last_seen_at) = last_seen_at {
        connection.insert(
            "last_seen_at".to_string(),
            Value::String(
                DateTime::<Utc>::from_naive_utc_and_offset(last_seen_at, Utc).to_rfc3339(),
            ),
        );
    }
    Value::Object(connection)
}

pub fn network_analyzer_connections(snapshot_json: &str) -> Option<Value> {
    let hosts = network_analyzer_hosts(snapshot_json)?;

    let connections: Vec<Value> = hosts
        .iter()
        .map(|host| network_host_connection(host, "active", None, None))
        .collect();

    Some(Value::Array(connections))
}

pub fn network_analyzer_hosts_from_metadata(
    metadata: &HashMap<String, String>,
) -> Option<Vec<ObservedNetworkHost>> {
    if metadata.get("kind").map(String::as_str) == Some("network_analyzer_scan") {
        if let Some(snapshot_json) = metadata.get("snapshot_json") {
            return network_analyzer_hosts(snapshot_json);
        }
    }

    None
}

pub fn declared_connections_from_metadata(metadata: &HashMap<String, String>) -> Option<Value> {
    for key in ["connections_json", "declared_connections", "connections"] {
        if let Some(raw) = metadata.get(key) {
            match serde_json::from_str::<Value>(raw) {
                Ok(value) => return normalize_connection_array(&value),
                Err(e) => {
                    warn!("Ignoring malformed declared connection metadata '{key}': {e}");
                    return None;
                }
            }
        }
    }

    if metadata.get("kind").map(String::as_str) == Some("network_analyzer_scan") {
        if let Some(snapshot_json) = metadata.get("snapshot_json") {
            return network_analyzer_connections(snapshot_json);
        }
    }

    None
}

pub fn declared_connections_from_custom_json(custom_json: &Value) -> Option<Value> {
    let metadata = custom_json.as_object()?;

    for key in ["connections_json", "declared_connections", "connections"] {
        if let Some(raw) = metadata.get(key).and_then(Value::as_str) {
            match serde_json::from_str::<Value>(raw) {
                Ok(value) => return normalize_connection_array(&value),
                Err(_) => return None,
            }
        }
    }

    if metadata.get("kind").and_then(Value::as_str) == Some("network_analyzer_scan") {
        if let Some(snapshot_json) = metadata.get("snapshot_json").and_then(Value::as_str) {
            return network_analyzer_connections(snapshot_json);
        }
    }

    None
}
