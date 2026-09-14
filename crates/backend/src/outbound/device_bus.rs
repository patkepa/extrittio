use crate::state::ZenohMetrics;
use async_trait::async_trait;
use extrittio_backend_core::commands::{CommandDelivery, DeviceBus, DeviceBusError};
use prost::Message;
use std::sync::{Arc, atomic::Ordering};

pub struct ZenohDeviceBus {
    session: Arc<zenoh::Session>,
    metrics: Arc<ZenohMetrics>,
}
impl ZenohDeviceBus {
    pub fn new(session: Arc<zenoh::Session>, metrics: Arc<ZenohMetrics>) -> Self {
        Self { session, metrics }
    }
}
#[async_trait]
impl DeviceBus for ZenohDeviceBus {
    fn supports(&self, protocol: &extrittio_device_contract::TransportProtocol) -> bool {
        *protocol == extrittio_device_contract::TransportProtocol::Zenoh
    }
    async fn publish_command(&self, delivery: CommandDelivery) -> Result<(), DeviceBusError> {
        let params = delivery
            .params
            .as_object()
            .ok_or_else(|| DeviceBusError("Command params must be a JSON object".into()))?
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| value.to_string()),
                )
            })
            .collect();
        let topic = delivery
            .address
            .unwrap_or_else(|| extrittio_common::topics::commands(&delivery.device_id));
        let payload = extrittio_common::extrittio::DeviceCommand {
            command: delivery.command,
            params,
            correlation_id: delivery.correlation_id,
        }
        .encode_to_vec();
        self.session.put(topic, payload).await.map_err(|error| {
            tracing::warn!("Command publication failed: {error}");
            DeviceBusError(error.to_string())
        })?;
        self.metrics.messages_out.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
