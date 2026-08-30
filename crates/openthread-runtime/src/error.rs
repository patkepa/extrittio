use std::{io, path::PathBuf};

/// Errors produced while discovering, supervising, or controlling local OpenThread.
#[derive(Debug, thiserror::Error)]
pub enum OpenThreadError {
    #[error("The local OpenThread border router is unavailable: {0}")]
    Unavailable(String),
    #[error("Invalid OpenThread configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Invalid Thread operational dataset: {0}")]
    InvalidDataset(String),
    #[error("Unable to discover OpenThread runtime components: {0}")]
    Discovery(String),
    #[error("OpenThread process failure: {0}")]
    Process(String),
    #[error("OpenThread control request failed: {0}")]
    Control(String),
    #[error("OpenThread returned an invalid response: {0}")]
    InvalidResponse(String),
    #[error("The Thread network changed while the operation was running")]
    NetworkChanged,
    #[error("{operation} at {}: {source}", path.display())]
    FileSystem {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("OpenThread HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("OpenThread D-Bus request failed: {0}")]
    Dbus(#[from] zbus::Error),
    #[error("OpenThread D-Bus property request failed: {0}")]
    DbusProperty(#[from] zbus::fdo::Error),
}

pub type Result<T> = std::result::Result<T, OpenThreadError>;
