use std::path::PathBuf;

/// Database engines that can back Extrittio persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Postgres,
    Turso,
}

impl BackendKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Turso => "turso",
        }
    }
}

/// Operational capabilities that may tune worker behavior without changing
/// which product features are available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub multi_process: bool,
    pub concurrent_claimers: bool,
    pub partitioned_telemetry: bool,
}

/// Safe, backend-neutral database information shared by application state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDescriptor {
    pub kind: BackendKind,
    pub capabilities: BackendCapabilities,
    pub local_file: Option<PathBuf>,
}

impl BackendDescriptor {
    #[must_use]
    pub fn postgres() -> Self {
        Self {
            kind: BackendKind::Postgres,
            capabilities: BackendCapabilities {
                multi_process: true,
                concurrent_claimers: true,
                partitioned_telemetry: true,
            },
            local_file: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_descriptor_preserves_production_capabilities() {
        let descriptor = BackendDescriptor::postgres();

        assert_eq!(descriptor.kind, BackendKind::Postgres);
        assert!(descriptor.capabilities.multi_process);
        assert!(descriptor.capabilities.concurrent_claimers);
        assert!(descriptor.capabilities.partitioned_telemetry);
        assert!(descriptor.local_file.is_none());
    }
}
