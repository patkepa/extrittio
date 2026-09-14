#[cfg(feature = "otlp")]
use opentelemetry::{global, propagation::Injector};
use std::time::Duration;
#[cfg(feature = "otlp")]
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub struct HttpWebhookSender;
#[async_trait::async_trait]
impl extrittio_backend_core::rule_actions::WebhookSender for HttpWebhookSender {
    async fn send(
        &self,
        url: &str,
        headers: &std::collections::HashMap<String, String>,
        payload: &serde_json::Value,
        delivery_id: &str,
    ) -> Result<(), String> {
        let parsed_url = crate::security::validate_public_https_url(url, "webhook url")
            .map_err(|e| e.to_string())?;
        let resolved_addrs = crate::security::validate_resolved_public_target(&parsed_url).await?;
        let host = parsed_url
            .host_str()
            .ok_or_else(|| "webhook url must include a host".to_string())?;
        let pinned_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(host, &resolved_addrs)
            .build()
            .map_err(|e| e.to_string())?;
        let mut req = pinned_client.post(url).json(payload);
        for (key, value) in headers {
            req = req.header(key, value);
        }
        req = req
            .header("Idempotency-Key", delivery_id)
            .header("X-Extrittio-Event-ID", delivery_id);
        #[cfg(feature = "otlp")]
        {
            let mut trace_headers = reqwest::header::HeaderMap::new();
            global::get_text_map_propagator(|propagator| {
                propagator.inject_context(
                    &tracing::Span::current().context(),
                    &mut HeaderInjector(&mut trace_headers),
                );
            });
            req = req.headers(trace_headers);
        }
        let resp = req
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "webhook to {url} returned status {}",
                resp.status()
            ))
        }
    }
}

#[cfg(feature = "otlp")]
struct HeaderInjector<'a>(&'a mut reqwest::header::HeaderMap);

#[cfg(feature = "otlp")]
impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) else {
            return;
        };
        let Ok(value) = reqwest::header::HeaderValue::from_str(&value) else {
            return;
        };
        self.0.insert(name, value);
    }
}
