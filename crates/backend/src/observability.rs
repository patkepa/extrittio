use std::env;

use anyhow::{Context, Result, bail};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// Owns exporters that need an explicit flush during graceful shutdown.
pub struct ObservabilityGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl ObservabilityGuard {
    pub fn shutdown(mut self) -> Result<()> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Result<()> {
        if let Some(provider) = self.tracer_provider.take() {
            provider
                .shutdown()
                .context("failed to flush OpenTelemetry traces")?;
        }
        Ok(())
    }
}

impl Drop for ObservabilityGuard {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_inner() {
            eprintln!("failed to shut down OpenTelemetry: {error:#}");
        }
    }
}

/// Installs structured local logging and, when an OTLP endpoint is explicitly
/// configured, distributed tracing export.
///
/// Set `OTEL_EXPORTER_OTLP_ENDPOINT` (or the trace-specific variant) to enable
/// OTLP/HTTP export. Set `EXTRITTIO_LOG_FORMAT=json` for machine-readable logs.
pub fn init(service_name: &str, default_filter: &str) -> Result<ObservabilityGuard> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let tracer_provider = build_tracer_provider(service_name)?;
    let log_format = env::var("EXTRITTIO_LOG_FORMAT")
        .unwrap_or_else(|_| "text".to_string())
        .to_ascii_lowercase();

    match log_format.as_str() {
        "json" => {
            let otel_layer = tracer_provider.as_ref().map(|provider| {
                tracing_opentelemetry::layer()
                    .with_tracer(provider.tracer(service_name.to_string()))
            });
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().json())
                .with(otel_layer)
                .try_init()
                .context("failed to initialize tracing subscriber")?;
        }
        "text" | "pretty" => {
            let otel_layer = tracer_provider.as_ref().map(|provider| {
                tracing_opentelemetry::layer()
                    .with_tracer(provider.tracer(service_name.to_string()))
            });
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .with(otel_layer)
                .try_init()
                .context("failed to initialize tracing subscriber")?;
        }
        other => bail!("EXTRITTIO_LOG_FORMAT must be `text`, `pretty`, or `json`, got `{other}`"),
    }

    Ok(ObservabilityGuard { tracer_provider })
}

fn build_tracer_provider(service_name: &str) -> Result<Option<SdkTracerProvider>> {
    if otel_disabled() || !otel_endpoint_configured() {
        return Ok(None);
    }

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
        .context("failed to configure the OTLP trace exporter")?;
    let resource = Resource::builder()
        .with_service_name(service_name.to_string())
        .build();
    let provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build();
    global::set_tracer_provider(provider.clone());
    Ok(Some(provider))
}

fn otel_endpoint_configured() -> bool {
    [
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
    ]
    .iter()
    .any(|key| env::var(key).is_ok_and(|value| !value.trim().is_empty()))
}

fn otel_disabled() -> bool {
    env::var("OTEL_SDK_DISABLED")
        .is_ok_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1"))
}
