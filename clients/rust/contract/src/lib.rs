#[cfg(feature = "async-fs")]
use std::path::Path;
use std::time::Duration;

use extrittio_device_contract::{
    CONTRACT_API_VERSION, CompiledContractDocument, CompiledRoute, ContractHash, PayloadEncoding,
    RouteDirection, SchemaFormat, SchemaProfile, TransportProtocol, validate_instance,
};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum ProvisionedContractError {
    #[error("failed to read provisioned contract: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid provisioned contract JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("provisioned contract response has no document")]
    MissingDocument,
    #[error("provisioned contract response has no contract hash")]
    MissingHash,
    #[error("unsupported contract API {0}")]
    UnsupportedApi(u32),
    #[error("provisioned contract hash does not match its document")]
    HashMismatch,
    #[error("provisioned contract belongs to device '{actual}', not '{expected}'")]
    DeviceMismatch { expected: String, actual: String },
    #[error("provisioned contract has no Zenoh transport")]
    MissingZenohTransport,
    #[error("stream '{0}' is not declared by the provisioned contract")]
    UnknownStream(String),
    #[error("route '{0}' is not declared by the provisioned contract")]
    UnknownRoute(String),
    #[error("contract canonicalization failed: {0}")]
    Canonicalization(String),
    #[error("stream '{0}' is not a device-to-cloud JSON stream")]
    UnsupportedEventStream(String),
    #[error("event payload failed contract validation: {0}")]
    InvalidPayload(String),
    #[error("event envelope serialization failed: {0}")]
    EventSerialization(String),
    #[error("event envelope is {actual} bytes; contract permits {maximum}")]
    EventTooLarge { actual: usize, maximum: u64 },
}

#[derive(Debug, Clone)]
pub struct ContractEvent {
    pub stream_key: String,
    pub payload: Value,
}

impl ContractEvent {
    #[must_use]
    pub fn new(stream_key: impl Into<String>, payload: Value) -> Self {
        Self {
            stream_key: stream_key.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedContractEvent {
    pub address: String,
    pub payload: Vec<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceEventEnvelope<'a> {
    api_version: u32,
    event_id: String,
    contract_hash: String,
    occurred_at: chrono::DateTime<chrono::Utc>,
    payload: &'a Value,
}

/// Verified, immutable runtime view of the contract downloaded during
/// provisioning. The device runtime consumes this instead of knowing model-
/// specific endpoints, topics, commands, or timing defaults.
#[derive(Debug, Clone)]
pub struct ProvisionedContract {
    document: CompiledContractDocument,
    hash: ContractHash,
}

impl ProvisionedContract {
    pub fn location_max_age(&self) -> Option<Duration> {
        self.document
            .location
            .as_ref()
            .map(|binding| Duration::from_millis(binding.max_age_ms))
    }

    #[cfg(feature = "async-fs")]
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, ProvisionedContractError> {
        let bytes = tokio::fs::read(path).await?;
        Self::from_api_response(&bytes)
    }

    pub fn from_api_response(bytes: &[u8]) -> Result<Self, ProvisionedContractError> {
        let envelope: Value = serde_json::from_slice(bytes)?;
        let document = envelope
            .get("document")
            .cloned()
            .ok_or(ProvisionedContractError::MissingDocument)?;
        let expected_hash = envelope
            .get("contract_hash")
            .or_else(|| envelope.get("contractHash"))
            .and_then(Value::as_str)
            .ok_or(ProvisionedContractError::MissingHash)?;
        let document: CompiledContractDocument = serde_json::from_value(document)?;
        Self::verify(document, expected_hash)
    }

    pub fn verify(
        document: CompiledContractDocument,
        expected_hash: &str,
    ) -> Result<Self, ProvisionedContractError> {
        if document.contract_api == 0 || document.contract_api > CONTRACT_API_VERSION {
            return Err(ProvisionedContractError::UnsupportedApi(
                document.contract_api,
            ));
        }
        let hash = document
            .contract_hash()
            .map_err(|error| ProvisionedContractError::Canonicalization(error.to_string()))?;
        if hash.to_string() != expected_hash {
            return Err(ProvisionedContractError::HashMismatch);
        }
        Ok(Self { document, hash })
    }

    pub fn validate_device_id(&self, device_id: &str) -> Result<(), ProvisionedContractError> {
        if self.document.device_id == device_id {
            Ok(())
        } else {
            Err(ProvisionedContractError::DeviceMismatch {
                expected: device_id.to_string(),
                actual: self.document.device_id.clone(),
            })
        }
    }

    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.document.device_id
    }

    #[must_use]
    pub fn contract_id(&self) -> &str {
        &self.document.contract_id
    }

    #[must_use]
    pub fn hash(&self) -> &ContractHash {
        &self.hash
    }

    pub fn zenoh_endpoint(&self) -> Result<&str, ProvisionedContractError> {
        self.document
            .transports
            .values()
            .find(|transport| transport.protocol == TransportProtocol::Zenoh)
            .map(|transport| transport.endpoint.as_str())
            .ok_or(ProvisionedContractError::MissingZenohTransport)
    }

    #[must_use]
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.document.runtime.heartbeat_interval_ms)
    }

    pub fn route_for_stream(
        &self,
        stream_key: &str,
    ) -> Result<&CompiledRoute, ProvisionedContractError> {
        let stream = self
            .document
            .streams
            .get(stream_key)
            .ok_or_else(|| ProvisionedContractError::UnknownStream(stream_key.to_string()))?;
        self.route(&stream.route)
    }

    pub fn route(&self, route_key: &str) -> Result<&CompiledRoute, ProvisionedContractError> {
        self.document
            .routes
            .get(route_key)
            .ok_or_else(|| ProvisionedContractError::UnknownRoute(route_key.to_string()))
    }

    #[must_use]
    pub fn document(&self) -> &CompiledContractDocument {
        &self.document
    }

    pub fn encode_event(
        &self,
        event: &ContractEvent,
        occurred_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<EncodedContractEvent, ProvisionedContractError> {
        let route = self.route_for_stream(&event.stream_key)?;
        if route.direction != RouteDirection::DeviceToCloud
            || route.encoding != PayloadEncoding::Json
        {
            return Err(ProvisionedContractError::UnsupportedEventStream(
                event.stream_key.clone(),
            ));
        }
        let schema = self
            .document
            .schemas
            .get(&route.message_schema)
            .filter(|schema| schema.format == SchemaFormat::JsonSchema)
            .ok_or_else(|| {
                ProvisionedContractError::UnsupportedEventStream(event.stream_key.clone())
            })?;
        validate_instance(SchemaProfile::ExtrittioV1, &schema.schema, &event.payload)
            .map_err(|error| ProvisionedContractError::InvalidPayload(error.to_string()))?;
        let payload = serde_json::to_vec(&DeviceEventEnvelope {
            api_version: CONTRACT_API_VERSION,
            event_id: uuid::Uuid::new_v4().to_string(),
            contract_hash: self.hash.to_string(),
            occurred_at,
            payload: &event.payload,
        })
        .map_err(|error| ProvisionedContractError::EventSerialization(error.to_string()))?;
        if payload.len() as u64 > self.document.runtime.max_message_bytes {
            return Err(ProvisionedContractError::EventTooLarge {
                actual: payload.len(),
                maximum: self.document.runtime.max_message_bytes,
            });
        }
        Ok(EncodedContractEvent {
            address: route.address.clone(),
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> CompiledContractDocument {
        serde_json::from_value(serde_json::json!({
            "contractApi": 1,
            "contractId": "contract-1",
            "tenantId": "tenant-1",
            "deviceId": "device-1",
            "blueprintRevisionId": "revision-1",
            "blueprintRevision": 1,
            "blueprintKey": "sensor",
            "blueprintName": "Sensor",
            "runtime": {
                "heartbeatIntervalMs": 30000,
                "offlineAfterMs": 95000,
                "maxMessageBytes": 8192,
                "maxMessagesPerMinute": 120,
                "maxMetricCardinality": 128
            },
            "transports": {
                "primary": {
                    "protocol": "zenoh",
                    "endpoint": "tls/hub.example.test:7447",
                    "delivery": "at_least_once",
                    "ordering": "per_device_stream"
                }
            },
            "routes": {
                "readings": {
                    "transport": "primary",
                    "direction": "device_to_cloud",
                    "address": "extrittio/devices/device-1/events/readings",
                    "messageSchema": "reading@1",
                    "encoding": "json"
                }
            },
            "schemas": {},
            "streams": {
                "environment": {"route": "readings", "timestamp": "envelope.occurred_at", "fields": {}}
            },
            "commands": {},
            "relationships": []
        }))
        .unwrap()
    }

    #[test]
    fn verifies_envelope_and_resolves_runtime_values() {
        let document = document();
        let hash = document.contract_hash().unwrap();
        let envelope = serde_json::json!({
            "contract_hash": hash.to_string(),
            "document": document
        });
        let contract =
            ProvisionedContract::from_api_response(&serde_json::to_vec(&envelope).unwrap())
                .unwrap();
        assert_eq!(contract.device_id(), "device-1");
        assert_eq!(
            contract.zenoh_endpoint().unwrap(),
            "tls/hub.example.test:7447"
        );
        assert_eq!(contract.heartbeat_interval(), Duration::from_secs(30));
        assert_eq!(
            contract.route_for_stream("environment").unwrap().address,
            "extrittio/devices/device-1/events/readings"
        );
    }

    #[test]
    fn rejects_tampered_contracts_and_wrong_devices() {
        let document = document();
        assert!(matches!(
            ProvisionedContract::verify(document.clone(), &"0".repeat(64)),
            Err(ProvisionedContractError::HashMismatch)
        ));
        let hash = document.contract_hash().unwrap();
        let contract = ProvisionedContract::verify(document, &hash.to_string()).unwrap();
        assert!(matches!(
            contract.validate_device_id("another-device"),
            Err(ProvisionedContractError::DeviceMismatch { .. })
        ));
    }

    #[test]
    fn rejects_missing_contract_and_unsupported_api() {
        assert!(matches!(
            ProvisionedContract::from_api_response(b"{}"),
            Err(ProvisionedContractError::MissingDocument)
        ));
        let mut document = document();
        document.contract_api = CONTRACT_API_VERSION + 1;
        let hash = document.contract_hash().unwrap();
        assert!(matches!(
            ProvisionedContract::verify(document, &hash.to_string()),
            Err(ProvisionedContractError::UnsupportedApi(_))
        ));
    }

    #[test]
    fn validates_and_encodes_contract_event() {
        let mut document = document();
        document.schemas.insert(
            "reading@1".to_string(),
            serde_json::from_value(serde_json::json!({
                "format": "json_schema",
                "schema": {
                    "type": "object",
                    "properties": {"temperature": {"type": "number"}},
                    "required": ["temperature"],
                    "additionalProperties": false
                }
            }))
            .unwrap(),
        );
        let hash = document.contract_hash().unwrap();
        let contract = ProvisionedContract::verify(document, &hash.to_string()).unwrap();
        let encoded = contract
            .encode_event(
                &ContractEvent::new("environment", serde_json::json!({"temperature": 21.5})),
                chrono::DateTime::UNIX_EPOCH,
            )
            .unwrap();
        assert_eq!(
            encoded.address,
            "extrittio/devices/device-1/events/readings"
        );
        let envelope: Value = serde_json::from_slice(&encoded.payload).unwrap();
        assert_eq!(envelope["contractHash"], contract.hash().to_string());
        assert_eq!(envelope["payload"]["temperature"], 21.5);

        assert!(matches!(
            contract.encode_event(
                &ContractEvent::new("undeclared", serde_json::json!({})),
                chrono::DateTime::UNIX_EPOCH
            ),
            Err(ProvisionedContractError::UnknownStream(_))
        ));
        assert!(matches!(
            contract.encode_event(
                &ContractEvent::new("environment", serde_json::json!({"humidity": 50})),
                chrono::DateTime::UNIX_EPOCH
            ),
            Err(ProvisionedContractError::InvalidPayload(_))
        ));
        let mut tiny = contract.document().clone();
        tiny.runtime.max_message_bytes = 1;
        let hash = tiny.contract_hash().unwrap();
        let tiny = ProvisionedContract::verify(tiny, &hash.to_string()).unwrap();
        assert!(matches!(
            tiny.encode_event(
                &ContractEvent::new("environment", serde_json::json!({"temperature": 21.5})),
                chrono::DateTime::UNIX_EPOCH
            ),
            Err(ProvisionedContractError::EventTooLarge { .. })
        ));
    }
}
