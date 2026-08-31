#[derive(Debug, thiserror::Error)]
pub enum TursoLifecycleError {
    #[error("Turso database is unavailable: {0}")]
    Unavailable(String),

    #[error("Turso migration failed: {0}")]
    Migration(String),

    #[error("Turso database returned corrupt data: {0}")]
    CorruptData(String),

    #[error("internal Turso lifecycle failure: {0}")]
    Internal(String),
}

pub(crate) fn map_open_error(error: turso::Error) -> TursoLifecycleError {
    match error {
        turso::Error::Corrupt(message) | turso::Error::NotAdb(message) => {
            TursoLifecycleError::CorruptData(message)
        }
        other => TursoLifecycleError::Unavailable(other.to_string()),
    }
}

pub(crate) fn map_operation_error(error: turso::Error) -> TursoLifecycleError {
    match error {
        turso::Error::Corrupt(message)
        | turso::Error::NotAdb(message)
        | turso::Error::ConversionFailure(message) => TursoLifecycleError::CorruptData(message),
        turso::Error::IoError(_, _)
        | turso::Error::Readonly(_)
        | turso::Error::DatabaseFull(_)
        | turso::Error::Busy(_)
        | turso::Error::BusySnapshot(_) => TursoLifecycleError::Unavailable(error.to_string()),
        other => TursoLifecycleError::Internal(other.to_string()),
    }
}

pub(crate) fn map_migration_error(error: turso::Error) -> TursoLifecycleError {
    TursoLifecycleError::Migration(error.to_string())
}
