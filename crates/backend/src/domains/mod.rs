//! Vertical domain boundaries for the modular monolith.
//!
//! The legacy `api`, `services`, and `repositories` module paths remain as
//! compatibility aliases while callers migrate to these cohesive boundaries.

pub mod alerts {
    #[path = "repository.rs"]
    pub mod port;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::alerts as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::alert_repo as repository;
    pub use crate::services::alert_service as service;
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
    pub use crate::services::device_catalog_service as catalog_service;
    pub use crate::services::device_ingress_service as ingress_service;
    pub use crate::services::device_service as service;
}

pub mod events {
    pub mod repository;
    pub mod service;
    pub mod types;
}

pub mod device_blueprints {
    pub mod blueprint_service;
    pub mod repository;
    pub mod types;

    pub use crate::api::device_blueprints as api;
}

pub mod device_types {
    #[path = "repository.rs"]
    pub mod repository;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::device_types as api;
    pub use crate::services::device_type_service as service;
}

pub mod firmware {
    #[path = "repository.rs"]
    pub mod port;
    #[path = "types.rs"]
    pub mod types;

    pub use super::firmware_store as storage;
    pub use crate::api::ci_pipeline as ci_api;
    pub use crate::api::firmware_updates as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::firmware_repo as repository;
    pub use crate::services::ci_pipeline_service as ci_service;
    pub use crate::services::firmware_service as service;
}

#[path = "firmware/firmware_store.rs"]
pub mod firmware_store;

pub mod fleets {
    #[path = "repository.rs"]
    pub mod repository;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::fleets as api;
    pub use crate::services::fleet_service as service;
}

pub mod identity {
    #[path = "api_key_repository.rs"]
    pub mod api_key_repository;
    #[path = "api_key_types.rs"]
    pub mod api_key_types;
    #[path = "certificate_repository.rs"]
    pub mod certificate_repository;
    #[path = "certificate_types.rs"]
    pub mod certificate_types;
    #[path = "role_repository.rs"]
    pub mod role_repository;
    #[path = "role_types.rs"]
    pub mod role_types;
    #[path = "user_repository.rs"]
    pub mod user_repository;
    #[path = "user_types.rs"]
    pub mod user_types;

    pub use crate::api::{api_keys, auth_routes, certificates, roles, users};
    #[cfg(feature = "postgres")]
    pub use crate::repositories::{api_key_repo, cert_repo, role_repo, user_repo};
    pub use crate::services::{api_key_service, cert_service, role_service, user_service};
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

    #[path = "outbox_repository.rs"]
    pub mod outbox_repository;
    #[path = "outbox_types.rs"]
    pub mod outbox_types;

    pub use crate::api::{health, outbox, server_metrics, system};
    #[cfg(feature = "postgres")]
    pub use crate::repositories::{rule_action_outbox_repo, server_metrics_repo};
    pub use crate::services::{metrics_middleware, server_metrics as server_metrics_service};
}

pub mod rules {
    #[path = "repository.rs"]
    pub mod port;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::rules as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::rule_repo as repository;
    pub use crate::rule_engine as engine;
    pub use crate::services::rule_service as service;
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
    #[path = "repository.rs"]
    pub mod port;
    #[path = "types.rs"]
    pub mod types;

    pub use crate::api::zones as api;
    #[cfg(feature = "postgres")]
    pub use crate::repositories::zone_repo as repository;
    pub use crate::services::zone_service as service;
}
