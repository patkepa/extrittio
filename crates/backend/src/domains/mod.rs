//! Vertical domain boundaries for the modular monolith.
//!
//! The legacy `api`, `services`, and `repositories` module paths remain as
//! compatibility aliases while callers migrate to these cohesive boundaries.

pub mod alerts {
    pub use crate::api::alerts as api;
}

pub mod analytics {
    pub mod analytics_service;
    pub mod repository;
    pub mod types;

    pub use crate::api::analytics as api;
}

pub mod activity {
    pub mod repository;
    pub mod types;

    pub use crate::api::activity as api;
    pub use crate::services::activity_service as service;
}

pub mod audit {
    #[path = "repository.rs"]
    pub mod port;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::audit as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::audit_repo as repository;
    pub use crate::services::audit_service as service;
}

pub mod commands {
    #[path = "repository.rs"]
    pub mod port;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::commands as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::command_repo as repository;
    pub use crate::services::command_service as service;
}

pub mod configuration {
    #[path = "repository.rs"]
    pub mod repository;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::configs as api;
    pub use crate::services::config_service as service;
}

pub mod dashboard {
    #[path = "repository.rs"]
    pub mod repository;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::dashboard as api;
    pub use crate::services::dashboard_service as service;
}

pub mod devices {
    #[path = "repository.rs"]
    pub mod repository;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::devices as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::device_repo as legacy_repository;
    pub use crate::services::device_ingress_service as ingress_service;
    pub use crate::services::device_service as service;
}

pub mod events {
    pub mod repository;
    pub mod service;
    pub mod types;
}

pub mod device_blueprints {

    pub use crate::api::device_blueprints as api;
}

pub mod device_types {
    pub use crate::api::device_types as api;
}

pub mod firmware {
    pub mod download;
    #[path = "repository.rs"]
    pub mod port;
    #[path = "types.rs"]
    pub mod types;

    pub use super::firmware_store as storage;
    pub use crate::api::ci_pipeline as ci_api;
    pub use crate::api::firmware_updates as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::firmware_repo as repository;
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
    #[path = "../log_repository.rs"]
    pub mod port;
    #[path = "../log_types.rs"]
    pub mod types;

    pub use crate::api::logs as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::log_repo as repository;
    pub use crate::services::log_service as service;
}

pub mod operations {
    #[path = "metrics_repository.rs"]
    pub mod metrics_repository;
    #[path = "metrics_types.rs"]
    pub mod metrics_types;

    pub use crate::api::{health, outbox, server_metrics, system};
    #[cfg(feature = "postgres")]
    pub use crate::repositories::server_metrics_repo;
    pub use crate::services::{metrics_middleware, server_metrics as server_metrics_service};
}

pub mod rules {

    pub use crate::api::rules as api;
    pub use crate::rule_engine as engine;
}

pub mod shadows {
    #[path = "repository.rs"]
    pub mod repository;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::shadows as api;
    pub use crate::services::shadow_service as service;
}

pub mod telemetry {
    #[path = "repository.rs"]
    pub mod port;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::telemetry as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::telemetry_repo as repository;
    pub use crate::services::telemetry_service as service;
}

pub mod zones {
    pub use crate::api::zones as api;
}
