pub mod backend;
pub mod error;
pub(crate) mod factory;
#[cfg(feature = "postgres")]
pub(crate) mod postgres;
pub mod runtime;
#[cfg(feature = "turso")]
pub(crate) mod turso;

pub use backend::{BackendCapabilities, BackendDescriptor, BackendKind};
pub use error::{ConstraintName, PersistenceError};
pub use runtime::{DatabaseHealth, DatabaseRuntime, LifecycleError};

/// Construction result; live runtime state retains only lifecycle handles.
/// Keeping the pair private prevents mixing repositories and an unrelated runtime.
pub struct DatabaseComposition {
    repositories: extrittio_backend_core::RepositorySetInput,
    runtime: DatabaseRuntime,
}
impl DatabaseComposition {
    #[cfg(any(feature = "postgres", feature = "turso"))]
    pub(crate) fn new(
        repositories: extrittio_backend_core::RepositorySetInput,
        runtime: DatabaseRuntime,
    ) -> Self {
        Self {
            repositories,
            runtime,
        }
    }
    pub(crate) fn repositories(&self) -> &extrittio_backend_core::RepositorySetInput {
        &self.repositories
    }
    pub fn runtime(&self) -> &DatabaseRuntime {
        &self.runtime
    }
    pub(crate) fn into_parts(
        self,
    ) -> (extrittio_backend_core::RepositorySetInput, DatabaseRuntime) {
        (self.repositories, self.runtime)
    }
}
