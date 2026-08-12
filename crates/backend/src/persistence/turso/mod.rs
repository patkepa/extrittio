mod bootstrap;
mod database;

use std::sync::Arc;

pub use database::TursoDatabase;

#[derive(Clone)]
pub struct TursoAdapter {
    database: Arc<TursoDatabase>,
}

impl TursoAdapter {
    #[must_use]
    pub fn new(database: Arc<TursoDatabase>) -> Self {
        Self { database }
    }
}
