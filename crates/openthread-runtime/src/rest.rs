use std::{
    collections::HashMap,
    io::Read as _,
    time::{Duration, Instant},
};

use reqwest::{Url, blocking::Client, header};
use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, de::DeserializeOwned};
use tracing::warn;

use crate::{
    error::{OpenThreadError, Result},
    types::ThreadMeshDevice,
};

const JSON_API: &str = "application/vnd.api+json";
const MAX_JSON_BODY_BYTES: u64 = 2 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MESH_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(12);

pub(crate) struct OtbrRest {
    endpoint: Url,
    client: Client,
    unauthenticated_client: Client,
}

impl OtbrRest {
    pub(crate) fn new(endpoint: Url, token: SecretString) -> Result<Self> {
        Self::new_with_timeouts(endpoint, token, CONNECT_TIMEOUT, REQUEST_TIMEOUT)
    }

    fn new_with_timeouts(
        endpoint: Url,
        token: SecretString,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let bearer = SecretString::new(format!("Bearer {}", token.expose_secret()));
        let mut authorization =
            header::HeaderValue::from_str(bearer.expose_secret()).map_err(|error| {
                OpenThreadError::InvalidConfiguration(format!(
                    "invalid OTBR REST authorization header: {error}"
                ))
            })?;
        authorization.set_sensitive(true);
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, authorization);
        let client = Client::builder()
            .default_headers(headers)
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()?;
        let unauthenticated_client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()?;
        Ok(Self {
            endpoint,
            client,
            unauthenticated_client,
        })
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.endpoint
            .join(path.trim_start_matches('/'))
            .map_err(|error| {
                OpenThreadError::InvalidConfiguration(format!(
                    "invalid OTBR REST path {path}: {error}"
                ))
            })
    }

    fn action_url(&self, action_id: &str) -> Result<Url> {
        let mut url = self.endpoint.clone();
        url.path_segments_mut()
            .map_err(|()| {
                OpenThreadError::InvalidConfiguration(
                    "OTBR REST endpoint cannot contain path segments".to_string(),
                )
            })?
            .extend(["api", "actions", action_id]);
        Ok(url)
    }

    pub(crate) fn health_check(&self) -> Result<()> {
        let url = self.url("node/state")?;
        self.client.get(url).send()?.error_for_status()?;
        Ok(())
    }

    pub(crate) fn verify_authentication(&self) -> Result<()> {
        let url = self.url("node/state")?;
        let unauthenticated_status = self.unauthenticated_client.get(url).send()?.status();
        if unauthenticated_status != reqwest::StatusCode::UNAUTHORIZED {
            return Err(OpenThreadError::InvalidConfiguration(format!(
                "otbr-agent REST authentication is not enforced (unauthenticated probe returned {unauthenticated_status})"
            )));
        }
        Ok(())
    }

    pub(crate) fn scan_mesh_devices(&self) -> Result<Vec<ThreadMeshDevice>> {
        let actions_url = self.url("api/actions")?;
        self.client
            .delete(actions_url.clone())
            .header(header::ACCEPT, JSON_API)
            .send()?
            .error_for_status()?;

        let devices_url = self.url("api/devices")?;
        self.client
            .delete(devices_url.clone())
            .header(header::ACCEPT, JSON_API)
            .send()?
            .error_for_status()?;

        let response = self
            .client
            .post(actions_url)
            .header(header::CONTENT_TYPE, JSON_API)
            .header(header::ACCEPT, JSON_API)
            .json(&serde_json::json!({
                "data": [{
                    "type": "updateDeviceCollectionTask",
                    "attributes": {
                        "maxAge": 0,
                        "maxRetries": 1,
                        "deviceCount": 200,
                        "timeout": 8
                    }
                }]
            }))
            .send()?
            .error_for_status()?;
        let response: JsonApiCollection = read_json(response)?;
        let action_id = response
            .data
            .first()
            .and_then(|action| action.id.as_deref())
            .ok_or_else(|| {
                OpenThreadError::InvalidResponse(
                    "OTBR mesh discovery did not return an action identifier".to_string(),
                )
            })?;
        let discovery_result = self.wait_for_mesh_discovery(action_id);
        if let Err(error) = self.delete_action(action_id) {
            warn!(%error, action_id, "Failed to remove completed OTBR mesh-discovery action");
        }
        discovery_result?;

        let devices = self
            .client
            .get(devices_url)
            .header(header::ACCEPT, JSON_API)
            .send()?
            .error_for_status()?;
        let devices: JsonApiCollection = read_json(devices)?;
        parse_thread_mesh_devices(devices)
    }

    fn wait_for_mesh_discovery(&self, action_id: &str) -> Result<()> {
        let action_url = self.action_url(action_id)?;
        let deadline = Instant::now() + MESH_DISCOVERY_TIMEOUT;
        loop {
            let response = self
                .client
                .get(action_url.clone())
                .header(header::ACCEPT, JSON_API)
                .send()?
                .error_for_status()?;
            let response: JsonApiDocument = read_json(response)?;
            let status = response.data.attributes.status.as_deref().ok_or_else(|| {
                OpenThreadError::InvalidResponse(
                    "OTBR mesh discovery status is missing".to_string(),
                )
            })?;
            match mesh_action_complete(status, Instant::now() >= deadline)? {
                true => return Ok(()),
                false => {
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        }
    }

    fn delete_action(&self, action_id: &str) -> Result<()> {
        self.client
            .delete(self.action_url(action_id)?)
            .header(header::ACCEPT, JSON_API)
            .send()?
            .error_for_status()?;
        Ok(())
    }
}

fn read_json<T: DeserializeOwned>(response: reqwest::blocking::Response) -> Result<T> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_JSON_BODY_BYTES)
    {
        return Err(OpenThreadError::InvalidResponse(
            "OTBR REST response exceeds the configured size limit".to_string(),
        ));
    }
    let mut body = Vec::new();
    response
        .take(MAX_JSON_BODY_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|error| {
            OpenThreadError::InvalidResponse(format!("unable to read OTBR REST response: {error}"))
        })?;
    if body.len() as u64 > MAX_JSON_BODY_BYTES {
        return Err(OpenThreadError::InvalidResponse(
            "OTBR REST response exceeds the configured size limit".to_string(),
        ));
    }
    serde_json::from_slice(&body).map_err(|error| {
        OpenThreadError::InvalidResponse(format!("OTBR returned invalid JSON: {error}"))
    })
}

fn mesh_action_complete(status: &str, timed_out: bool) -> Result<bool> {
    match status {
        "completed" => Ok(true),
        "failed" => Err(OpenThreadError::Control(
            "OTBR mesh discovery failed".to_string(),
        )),
        "stopped" => Err(OpenThreadError::Control(
            "OTBR mesh discovery was cancelled before completion".to_string(),
        )),
        "pending" | "active" if !timed_out => Ok(false),
        "pending" | "active" => Err(OpenThreadError::Control(
            "OTBR mesh discovery timed out".to_string(),
        )),
        other => Err(OpenThreadError::InvalidResponse(format!(
            "OTBR returned unknown mesh discovery status {other:?}"
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct JsonApiCollection {
    data: Vec<JsonApiResource>,
}

#[derive(Debug, Deserialize)]
struct JsonApiDocument {
    data: JsonApiResource,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonApiResource {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default = "default_resource_type")]
    resource_type: String,
    #[serde(default)]
    attributes: ResourceAttributes,
}

fn default_resource_type() -> String {
    "threadDevice".to_string()
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceAttributes {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    ext_address: Option<String>,
    #[serde(default)]
    ml_eid_iid: Option<String>,
    #[serde(default, deserialize_with = "string_or_strings")]
    omr_ipv6_address: Vec<String>,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    eui: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    mode: Option<DeviceMode>,
    #[serde(default)]
    rloc16: Option<String>,
    #[serde(default)]
    rloc_address: Option<String>,
    #[serde(default)]
    router_id: Option<u16>,
    #[serde(default)]
    router_count: Option<u16>,
    #[serde(default)]
    network_name: Option<String>,
    #[serde(default)]
    ext_pan_id: Option<String>,
    #[serde(default)]
    ba_id: Option<String>,
    #[serde(default)]
    ba_state: Option<String>,
    #[serde(default)]
    leader_data: Option<LeaderData>,
    #[serde(default)]
    created: Option<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceMode {
    #[serde(default)]
    full_thread_device: Option<bool>,
    #[serde(default)]
    rx_on_when_idle: Option<bool>,
    #[serde(default)]
    full_network_data: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaderData {
    #[serde(default)]
    partition_id: Option<u32>,
    #[serde(default)]
    leader_router_id: Option<u16>,
    #[serde(default)]
    data_version: Option<u16>,
    #[serde(default)]
    stable_data_version: Option<u16>,
}

fn string_or_strings<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    Ok(match Option::<OneOrMany>::deserialize(deserializer)? {
        None => Vec::new(),
        Some(OneOrMany::One(value)) if !value.is_empty() => vec![value],
        Some(OneOrMany::One(_)) => Vec::new(),
        Some(OneOrMany::Many(values)) => values
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect(),
    })
}

fn parse_thread_mesh_devices(document: JsonApiCollection) -> Result<Vec<ThreadMeshDevice>> {
    document
        .data
        .into_iter()
        .map(|item| {
            let id = item.id.ok_or_else(|| {
                OpenThreadError::InvalidResponse(
                    "OTBR mesh device is missing its identifier".to_string(),
                )
            })?;
            let attributes = item.attributes;
            let _unknown_attribute_count = attributes.extra.len();
            let mode = attributes.mode;
            let leader = attributes.leader_data;
            Ok(ThreadMeshDevice {
                id,
                is_border_router: item.resource_type == "threadBorderRouter",
                extended_address: nonempty(attributes.ext_address),
                mesh_local_eid_iid: nonempty(attributes.ml_eid_iid),
                omr_ipv6_addresses: attributes.omr_ipv6_address,
                hostname: nonempty(attributes.hostname),
                eui64: nonempty(attributes.eui),
                role: nonempty(attributes.role).or_else(|| nonempty(attributes.state)),
                full_thread_device: mode.as_ref().and_then(|mode| mode.full_thread_device),
                rx_on_when_idle: mode.as_ref().and_then(|mode| mode.rx_on_when_idle),
                full_network_data: mode.as_ref().and_then(|mode| mode.full_network_data),
                rloc16: nonempty(attributes.rloc16),
                rloc_address: nonempty(attributes.rloc_address),
                router_id: attributes.router_id,
                router_count: attributes.router_count,
                network_name: nonempty(attributes.network_name),
                extended_pan_id: nonempty(attributes.ext_pan_id),
                border_agent_id: nonempty(attributes.ba_id),
                border_agent_state: nonempty(attributes.ba_state),
                partition_id: leader.as_ref().and_then(|data| data.partition_id),
                leader_router_id: leader.as_ref().and_then(|data| data.leader_router_id),
                data_version: leader.as_ref().and_then(|data| data.data_version),
                stable_data_version: leader.and_then(|data| data.stable_data_version),
                created_at: nonempty(attributes.created),
                updated_at: nonempty(attributes.updated),
            })
        })
        .collect()
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write as _, net::TcpListener};

    #[test]
    fn every_rest_request_uses_the_private_bearer_credential() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for status in ["200 OK", "401 Unauthorized"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let read = stream.read(&mut request).unwrap();
                write!(stream, "HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n").unwrap();
                requests.push(String::from_utf8_lossy(&request[..read]).into_owned());
            }
            requests
        });
        let rest = OtbrRest::new(endpoint, SecretString::new("private-token".to_string())).unwrap();
        rest.health_check().unwrap();
        rest.verify_authentication().unwrap();
        let requests = server.join().unwrap();
        assert!(requests[0].contains("authorization: Bearer private-token\r\n"));
        assert!(!requests[1].to_ascii_lowercase().contains("authorization:"));
    }

    #[test]
    fn parses_pinned_otbr_mesh_inventory_contract() {
        let inventory = serde_json::from_value::<JsonApiCollection>(serde_json::json!({
            "data": [{
                "id": "96518e5497d5b9f3",
                "type": "threadBorderRouter",
                "attributes": {
                    "extAddress": "96518e5497d5b9f3",
                    "mlEidIid": "731f529f1266a17d",
                    "omrIpv6Address": "fd11:22::1",
                    "role": "leader",
                    "mode": {
                        "fullThreadDevice": true,
                        "rxOnWhenIdle": true,
                        "fullNetworkData": true
                    },
                    "networkName": "Extrittio-Thread",
                    "extPanId": "ef1398c2fd504b67",
                    "leaderData": {
                        "partitionId": 1794764107,
                        "dataVersion": 64,
                        "stableDataVersion": 63,
                        "leaderRouterId": 60
                    }
                }
            }]
        }))
        .unwrap();
        let devices = parse_thread_mesh_devices(inventory).unwrap();
        assert_eq!(
            devices[0].extended_pan_id.as_deref(),
            Some("ef1398c2fd504b67")
        );
        assert_eq!(devices[0].omr_ipv6_addresses, vec!["fd11:22::1"]);
        assert_eq!(devices[0].partition_id, Some(1_794_764_107));
    }

    #[test]
    fn stopped_action_is_not_a_success_state() {
        let response = serde_json::from_value::<JsonApiDocument>(serde_json::json!({
            "data": {"type": "updateDeviceCollectionTask", "attributes": {"status": "stopped"}}
        }))
        .unwrap();
        let status = response.data.attributes.status.as_deref().unwrap();
        assert!(mesh_action_complete(status, false).is_err());
    }

    #[test]
    fn action_identifier_is_encoded_as_one_path_segment() {
        let rest = OtbrRest::new(
            Url::parse("http://127.0.0.1:8081/").unwrap(),
            SecretString::new("test-token".to_string()),
        )
        .unwrap();
        let url = rest.action_url("task/1?state=completed").unwrap();
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:8081/api/actions/task%2F1%3Fstate=completed"
        );
    }

    #[test]
    fn hung_rest_response_is_bounded_by_the_request_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(300));
        });
        let rest = OtbrRest::new_with_timeouts(
            endpoint,
            SecretString::new("private-token".to_string()),
            Duration::from_millis(50),
            Duration::from_millis(75),
        )
        .unwrap();

        let started = Instant::now();
        assert!(rest.health_check().is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        server.join().unwrap();
    }
}
