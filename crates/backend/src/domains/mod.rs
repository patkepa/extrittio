//! Host transport modules grouped by domain.
//! Business types and repository ports are owned by extrittio-backend-core.

pub mod alerts {
    pub use crate::api::alerts as api;
}

pub mod analytics {
    pub use crate::api::analytics as api;
}

pub mod activity {
    pub use crate::api::activity as api;
}

pub mod audit {
    pub use crate::api::audit as api;
}

pub mod commands {
    pub use crate::api::commands as api;
}

pub mod configuration {
    pub use crate::api::configs as api;
}

pub mod dashboard {
    pub use crate::api::dashboard as api;
}

pub mod devices {
    pub use crate::api::devices as api;
    pub use crate::services::device_service as service;
}

pub mod device_blueprints {
    pub use crate::api::device_blueprints as api;
}

pub mod firmware {
    pub mod download;

    pub use super::firmware_store as storage;
    pub use crate::api::ci_pipeline as ci_api;
    pub use crate::api::firmware_updates as api;
    pub use crate::services::firmware_service as service;
}

#[path = "firmware/firmware_store.rs"]
pub mod firmware_store;

pub mod fleets {
    pub use crate::api::fleets as api;
}

pub mod identity {
    pub use crate::api::{api_keys, auth_routes, certificates, roles, users};
}

pub mod logs {
    pub use crate::api::logs as api;
}

pub mod operations {
    pub use crate::api::{health, outbox, server_metrics, system};
    pub use crate::services::{metrics_middleware, server_metrics as server_metrics_service};
}

pub mod rules {
    pub use crate::api::rules as api;
    pub use crate::rule_engine as engine;
}

pub mod shadows {
    pub use crate::api::shadows as api;
    pub use crate::services::shadow_service as service;
}

pub mod telemetry {
    pub use crate::api::telemetry as api;
}

pub mod zones {
    pub use crate::api::zones as api;
}
