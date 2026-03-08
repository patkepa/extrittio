use diesel::r2d2::{ConnectionManager, Pool};
use diesel::SqliteConnection;
use std::sync::Arc;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub struct AppState {
    pub db_pool: DbPool,
    pub zenoh_session: Arc<zenoh::Session>,
}
