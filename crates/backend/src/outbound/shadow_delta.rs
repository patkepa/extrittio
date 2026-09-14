use std::sync::Arc;

pub struct ZenohDesiredDeltaPublisher<'a> {
    pub session: &'a Arc<zenoh::Session>,
    pub metrics: &'a crate::state::ZenohMetrics,
}
#[async_trait::async_trait]
impl extrittio_backend_core::application::DesiredDeltaPublisher for ZenohDesiredDeltaPublisher<'_> {
    async fn publish(&self, device_id: &str, delta: &serde_json::Value, version: i32) {
        crate::services::shadow_service::publish_delta_if_nonempty(
            self.session,
            device_id,
            delta,
            version,
            self.metrics,
        )
        .await;
    }
}
