pub mod handlers;
pub mod subscriber;

/// Applications used by the device subscriber. Constructed once by host composition;
/// contains no persistence ports or concrete database handles.
#[derive(Clone)]
pub(crate) struct DeviceMessageApplications {
    pub identity: extrittio_backend_core::DeviceIngressApplication,
    pub telemetry: extrittio_backend_core::TelemetryIngressApplication,
    pub events: extrittio_backend_core::EventIngressApplication,
    pub contracts: extrittio_backend_core::application::ContractIngressApplication,
    pub logs: extrittio_backend_core::LogIngressApplication,
    pub commands: extrittio_backend_core::CommandWorkerApplication,
    pub shadows: extrittio_backend_core::DeviceShadowApplication,
    pub reports: extrittio_backend_core::application::DeviceReportApplication,
}
