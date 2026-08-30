//! Supervision and controlled access for a local OpenThread Border Router.
//!
//! The crate owns Extrittio's RCP discovery, `otbr-agent` lifecycle, private
//! D-Bus connection, loopback diagnostics client, operational-dataset safety,
//! and shared scan cache. Callers interact through [`ThreadRuntime`]; the raw
//! OTBR transports are intentionally private so runtime generation and cache
//! invalidation cannot be bypassed.

mod controller;
mod dataset;
mod dbus;
mod discovery;
mod error;
mod process;
mod rest;
mod runtime;
mod types;

pub use discovery::default_infrastructure_interface;
pub use error::{OpenThreadError, Result};
pub use runtime::ThreadRuntime;
pub use types::{
    CreateNetwork, DEFAULT_DEVELOPMENT_DATASET_TLVS, ThreadActiveDataset, ThreadChannelDiagnostics,
    ThreadMeshDevice, ThreadNetwork, ThreadObservationState, ThreadRadioStatistics,
    ThreadRcpCandidate, ThreadRcpConfidence, ThreadRole, ThreadRuntimeConfig, ThreadRuntimePhase,
    ThreadRuntimeSnapshot, ThreadScan, ThreadScanSnapshot, ThreadScanSource,
    ThreadScanSourceStatus, ThreadStatus,
};
